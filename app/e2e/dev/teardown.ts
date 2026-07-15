/**
 * `make dev` stack — logging + idempotent teardown registry. Pure move/extract
 * from `dev-full.ts`. `teardowns` is a live-binding array the entry pushes to.
 */
import { closeSync, openSync } from 'node:fs'
import { join } from 'node:path'

import { LOGS } from './env.ts'

/** Everything we must tear down on exit, in reverse order of creation. */
export const teardowns: Array<() => void | Promise<void>> = []
const logFds: number[] = []
let shuttingDown = false

export function log(msg: string): void {
  // eslint-disable-next-line no-console
  console.log(msg)
}

/** Open a truncating log file and return its numeric fd (spawn stdio needs an
 *  fd, not a freshly-created WriteStream whose fd is still null). */
export function openLog(name: string): number {
  const fd = openSync(join(LOGS, `${name}.log`), 'w')
  logFds.push(fd)
  return fd
}

/** Idempotent teardown: kill children, stop Postgres + surfpool, close logs. */
export async function runTeardowns(reason: string): Promise<void> {
  if (shuttingDown) return
  shuttingDown = true
  log(`\n[dev] ${reason} — tearing down…`)
  for (const t of teardowns.reverse()) {
    try {
      await t()
    } catch (e) {
      log(`[dev] teardown error (ignored): ${String(e)}`)
    }
  }
  for (const fd of logFds) {
    try {
      closeSync(fd)
    } catch {
      /* already closed */
    }
  }
}
