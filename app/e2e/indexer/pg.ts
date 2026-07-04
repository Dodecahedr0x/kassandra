/**
 * Ephemeral Postgres for the indexer e2e: `initdb` a throwaway cluster in a temp
 * dir, start it on a private port + unix socket, create a database, and tear it
 * all down afterwards. No system service, no leftover state.
 *
 * Finds the postgres binaries on PATH or under the usual Homebrew/apt locations
 * (`PG_BIN` overrides). Returns a `DATABASE_URL` the indexer binary consumes.
 */
import { spawn, spawnSync } from 'node:child_process'
import { existsSync, mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

function pgBinDir(): string {
  if (process.env.PG_BIN && existsSync(join(process.env.PG_BIN, 'initdb'))) return process.env.PG_BIN
  const candidates = [
    '/opt/homebrew/opt/postgresql@16/bin',
    '/opt/homebrew/opt/postgresql@15/bin',
    '/usr/local/opt/postgresql@16/bin',
    '/usr/local/opt/postgresql@15/bin',
    '/usr/lib/postgresql/16/bin',
    '/usr/lib/postgresql/15/bin',
    '/usr/bin',
  ]
  for (const dir of candidates) if (existsSync(join(dir, 'initdb'))) return dir
  // Fall back to PATH (spawnSync resolves it).
  const which = spawnSync('which', ['initdb'], { encoding: 'utf8' })
  if (which.status === 0) return which.stdout.trim().replace(/\/initdb\s*$/, '')
  throw new Error('postgres binaries not found — set PG_BIN to the dir containing initdb/pg_ctl')
}

export interface EphemeralPg {
  databaseUrl: string
  stop: () => void
}

/** Boot a throwaway Postgres and return its connection string + a stop(). */
export async function startEphemeralPg(port = 5599): Promise<EphemeralPg> {
  const bin = pgBinDir()
  const dataDir = mkdtempSync(join(tmpdir(), 'kass-idx-pg-'))
  const run = (cmd: string, args: string[]) => {
    const r = spawnSync(join(bin, cmd), args, { encoding: 'utf8' })
    if (r.status !== 0) throw new Error(`${cmd} failed: ${r.stderr || r.stdout}`)
    return r
  }

  // 1) init the cluster (trust auth locally; no fsync for speed).
  run('initdb', ['-D', dataDir, '-U', 'postgres', '--auth=trust', '--no-sync'])

  // 2) start it on a private TCP port bound to localhost only.
  const server = spawn(
    join(bin, 'postgres'),
    ['-D', dataDir, '-p', String(port), '-c', 'listen_addresses=127.0.0.1', '-c', 'fsync=off'],
    { stdio: 'ignore', detached: false },
  )

  // 3) wait until it accepts connections.
  const deadline = Date.now() + 30_000
  for (;;) {
    const r = spawnSync(join(bin, 'pg_isready'), ['-h', '127.0.0.1', '-p', String(port), '-U', 'postgres'])
    if (r.status === 0) break
    if (Date.now() > deadline) throw new Error('postgres did not become ready in 30s')
    await new Promise((res) => setTimeout(res, 300))
  }

  // 4) create the indexer database.
  run('createdb', ['-h', '127.0.0.1', '-p', String(port), '-U', 'postgres', 'indexer'])

  const databaseUrl = `postgres://postgres@127.0.0.1:${port}/indexer`
  const stop = () => {
    try {
      server.kill('SIGKILL')
    } catch {
      /* already gone */
    }
    try {
      rmSync(dataDir, { recursive: true, force: true })
    } catch {
      /* best effort */
    }
  }
  return { databaseUrl, stop }
}
