# NOTES — driving surfpool headless for the Kassandra E2E (T1 recon)

> Empirical findings against **surfpool 1.0.0** (`~/.local/bin/surfpool`),
> `solana-cli 3.1.7`, on this machine. Date: 2026-06-30.

## TL;DR — the working rig

1. Spawn a standalone simnet headless:
   ```
   surfpool start --no-tui --block-production-mode transaction --no-deploy --port 8899
   ```
   RPC on `http://127.0.0.1:8899` (WS :8900). `--no-deploy` disables the auto
   txtx deployment runbook (we deploy ourselves via a cheatcode). Block-per-tx
   gives deterministic progress.
2. Wait for readiness: poll JSON-RPC `getHealth` until it returns `"ok"` (a few
   hundred ms). `getVersion` returns `{"surfnet-version":"1.0.0", ...}`.
3. Deploy the LOCAL `.so` at the **fixed** program id — see below.
4. Drive it with the SDK over standard RPC (web3.js v3 `Connection`:
   `getLatestBlockhash` → build+sign `Transaction` → `sendRawTransaction` →
   poll `getAccountInfo`). Decode accounts with the SDK decoders.
5. Teardown: `SIGKILL` the child process. Nothing else to clean up (in-memory
   simnet; no on-disk ledger to wipe).

All of this is implemented in `sdk/test/surfpool/harness.ts` (the
`SurfpoolHarness`), proven by `sdk/test/surfpool/surfpool-smoke.test.ts`.

## Deploying the LOCAL program at the FIXED id — the key finding

The program id `KassVxvXUEPr5apSr2MqiGva4VFtJXyYLLDFS3f83nY` is hard-coded in the
program (`lib.rs` `declare`d) and in the SDK. `cargo build-sbf` emits a
**random** program keypair (`target/deploy/kassandra_program-keypair.json` →
`CahaNz...`), so `solana program deploy` / `--program-id <keypair>` **cannot**
land the program at the fixed vanity id (we don't hold its private key).

**Working method — the `surfnet_setAccount` cheatcode.** surfpool exposes a
`surfnet_setAccount` RPC that writes an account verbatim. Write the ELF as a
**non-upgradeable BPFLoader2 program account** (a BPFLoader2 program account IS
its ELF — the same model as `solana-test-validator --bpf-program <id> <path>`
and litesvm's `addProgramFromFile`):

```jsonc
// params: [pubkey, accountUpdate]
["KassVxvXUEPr5apSr2MqiGva4VFtJXyYLLDFS3f83nY", {
  "lamports": 5000000000,
  "owner": "BPFLoader2111111111111111111111111111111111",
  "executable": true,
  "data": "<HEX of kassandra_program.so>"   // NOTE: hex, NOT base64
}]
```

surfpool then JIT-loads and **executes** it. Verified: a bogus discriminant is
rejected by the real program (`invalid instruction data`, with the program's own
logs), and a full SDK-built `init_protocol` is accepted and the Protocol PDA
decodes correctly.

> Gotcha: the `data` field is **hex-encoded**, not base64. (Passing base64 fails
> with `Invalid hex data provided: Invalid character ...`.) Use
> `Buffer.from(elf).toString("hex")`.

## Standalone vs fork (hermetic?)

`surfpool start` with **no** `--network`/`--rpc-url` still logs
`Datasource connection successful. Epoch .../Slot ...` at boot — surfpool 1.0.0
defaults to a **mainnet datasource** and lazily fetches accounts on demand. So:

- Booting needs network reachable for the datasource handshake (not fully
  air-gapped).
- BUT all Kassandra state is **local**: the program is written via
  `surfnet_setAccount`, and we fabricate mints/token accounts/payers locally, so
  the core path never touches the datasource and is deterministic + fast.

This is the standalone (non-fork) core path. Forking MetaDAO programs for the
challenge path (T4) is `--network mainnet` (or `--rpc-url`), which lazily pulls
the deployed conditional-vault/AMM accounts.

## Cheatcode RPC methods available (for T3 seeding)

Probed on the running simnet (present = returns a result or an
`Invalid params`/`should have N argument(s)` error; absent = `Method not found`):

| method | params (observed) | use |
| --- | --- | --- |
| `surfnet_setAccount` | `[pubkey, {lamports?, owner?, executable?, data(hex)?}]` | write any account — programs, PDAs, fabricated state |
| `surfnet_setTokenAccount` | `≥3 args` | set an SPL token account (balance/owner/mint) directly |
| `surfnet_cloneProgramAccount` | `[..,..]` (tuple of 2) | clone a program from the datasource (T4 MetaDAO) |
| `surfnet_setSupply` | `[..]` (tuple of 1) | set a mint supply |
| `surfnet_setProgramAuthority` | `≥1 arg` | set/override a program's upgrade authority |
| `surfnet_timeTravel` | `[{absoluteSlot}]` (also epoch/slotIndex in result) | advance the clock/slot — **T3 phase windows** |
| `surfnet_pauseClock` / `surfnet_resumeClock` | none | freeze/resume slot progression |

Standard RPC also works: `requestAirdrop` (returns a sig; in block-per-tx mode
the balance settles within a poll), `getHealth`, `getVersion`, `getBalance`,
`getAccountInfo`, `getLatestBlockhash`, `sendRawTransaction`.

Not present: `surfnet_setEpoch`, `surfnet_getAccountProfile`, `svm_setAccount`,
`surfnet_setProgramAccount`, `surfnet_setProgramFromFile`, `rpc.discover`.

For T3 phase windows: the on-chain `now()` reads the Clock `unix_timestamp`.
`surfnet_timeTravel` moves the slot/epoch; confirm whether it also advances
`unix_timestamp` enough to cross `phase_ends_at`, or whether the real
slot→time relationship must be driven (revisit in T3).

## Runner base-URL override (T1 runner change)

`runner/src/anthropic.rs` now reads `ANTHROPIC_BASE_URL` (env). When set
non-empty it is treated as the API **base** and `/v1/messages` is appended
(mirrors the official Anthropic SDK); unset → the pinned
`https://api.anthropic.com/v1/messages` default, unchanged. Also
`AnthropicProvider::with_base_url(key, base)` for tests. This is how T2 will
point the REAL provider at a local mock Anthropic server.

## Running the gated suite

- Default (fast, offline, **excludes** surfpool): `cd sdk && pnpm test` → 72.
- Gated E2E (spawns surfpool, needs the built `.so`): `cd sdk && pnpm test:e2e`
  (sets `KASSANDRA_E2E=1`). Skips cleanly if surfpool / the `.so` are absent.
- Prereqs: `just build` (produces `target/deploy/kassandra_program.so`),
  surfpool on `PATH` (or `SURFPOOL_BIN`), network reachable for the datasource
  handshake at boot.

Gating mechanism: `sdk/vitest.config.ts` excludes `test/surfpool/**` unless
`KASSANDRA_E2E=1`, so the default `pnpm test` never imports/spawns surfpool.
