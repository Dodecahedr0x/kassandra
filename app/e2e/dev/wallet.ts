/**
 * `make dev` stack — dev-wallet loading. Pure move/extract from `dev-full.ts`.
 */
import { existsSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import { Keypair } from '@solana/web3.js'

import { log } from './teardown.ts'

/**
 * The dev wallet: by default the local Solana CLI keypair
 * (`~/.config/solana/id.json`, override path via `DEV_WALLET_KEYPAIR`), so you
 * transact from the wallet you already hold — no import step. Falls back to a
 * freshly generated (and printed) keypair when no local keypair file exists.
 * Returns the loaded keypair plus whether it came from disk.
 */
export async function loadDevWallet(): Promise<{ wallet: Keypair; fromFile: boolean }> {
  const path = process.env.DEV_WALLET_KEYPAIR || join(homedir(), '.config', 'solana', 'id.json')
  if (existsSync(path)) {
    try {
      const secret = Uint8Array.from(JSON.parse(readFileSync(path, 'utf8')) as number[])
      return { wallet: await Keypair.fromSecretKey(secret), fromFile: true }
    } catch (e) {
      log(`[dev] ⚠ could not read ${path} (${(e as Error).message}); generating an ephemeral wallet`)
    }
  }
  return { wallet: await Keypair.generate(), fromFile: false }
}
