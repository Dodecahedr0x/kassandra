/**
 * `make dev` stack — ports, paths, and derived service URLs. Pure move/extract
 * from `dev-full.ts` (the runnable `make dev` entry).
 */
import { join, resolve } from 'node:path'

export const SURFPOOL_PORT = 8899
export const INDEXER_PORT = 3111
export const APP_PORT = 5173

export const APP_DIR = process.cwd() // `pnpm --filter app exec` runs here
export const ROOT = resolve(APP_DIR, '..')
export const LOGS = join(ROOT, 'logs')
// The indexer is a WORKSPACE member, so `cargo build --manifest-path
// indexer/Cargo.toml` writes the binary to the workspace-root target/, NOT
// indexer/target/ (which doesn't exist — the pre-merge per-crate path).
export const INDEXER_BIN = join(ROOT, 'target', 'release', 'kassandra-indexer')
export const RUNNER_CONFIG = join(LOGS, 'runner.config.json')
export const WALLET_FILE = join(APP_DIR, 'e2e', '.wallet.json')

export const rpcUrl = `http://127.0.0.1:${SURFPOOL_PORT}`
// surfpool's websocket (accountSubscribe/programSubscribe) — bound explicitly to
// RPC port + 1 so the indexer's price subscriber has a deterministic ws url.
export const wsUrl = `ws://127.0.0.1:${SURFPOOL_PORT + 1}`
export const indexerUrl = `http://127.0.0.1:${INDEXER_PORT}`
export const appUrl = `http://localhost:${APP_PORT}`
