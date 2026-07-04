# Browser E2E (Playwright)

End-to-end tests that drive the **real dApp UI** in a browser against a local
**surfpool** validator, with a **script-funded mock wallet** (a real keypair that
signs + sends + confirms). One command spins up everything:

```bash
scripts/e2e-playwright.sh
```

It builds the program `.so` + the SDK + Chromium, then runs Playwright, whose
`globalSetup` boots surfpool, deploys the program, inits the protocol, mints
KASS/USDC, generates + funds the wallet keypair, and **seeds one oracle per action
into the phase where that action is legal**. The specs inject the keypair
(`window.__E2E_WALLET_SECRET__` → the real-signing e2e wallet, `VITE_E2E=1`),
perform the UI action, and assert the **persistent on-chain effect** (the UI
success line is transient — wiped by the post-write refetch, so we verify the
chain via `onchain.ts`).

There are **two** runs. The **default** (`scripts/e2e-playwright.sh`) is a fast
local simnet covering the whole non-challenge surface. The **forked** run
(`scripts/e2e-playwright-fork.sh`) boots surfpool **forking mainnet** (so MetaDAO's
deployed conditional-vault / AMM / futarchy programs are executable) in `clock`
block-production mode and drives the whole challenge-market cluster through the
browser. Each has its own config (`playwright.config.ts` / `playwright.fork.config.ts`),
port (8899 / 8940), and Vite server (5173 / 5174), so they never collide.

## Files

- `global-setup.ts` — boot surfpool + seed one oracle per action.
- `seed.ts` — reusable phase drivers (create → dispute → FactProposal → FactVoting
  → AiClaim → Challenge), + `keepWindowOpen` (patches `phase_ends_at` to the far
  future, since surfpool time-travel is forward-only and a window closed by later
  seeding can't be re-entered by rewinding the clock), + `fabricateKassDao` /
  `seedDeadendOracle` for the admin ops.
- `onchain.ts` — read/decode accounts, a forward clock set, and `patchProtocol`
  (fabricate the governance field an admin op is gated on).
- `fork/` — the forked challenge-market project (`global-setup.ts`, `onchain.ts`,
  `challenge.spec.ts`).
- `e2eWallet.tsx` (in `src/lib`) — the real-signing wallet.
- `*.spec.ts` — the tests.

## Coverage

Every action is driven through the real app UI and asserted on-chain. Together the
two runs cover the **entire protocol surface**.

**Default run** — non-challenge surface + the DAO/admin ops:

| Instruction | Spec | Status |
|---|---|---|
| `createOracle` | create.spec | ✅ |
| `propose` | writes.spec | ✅ |
| `submitFact` | writes.spec | ✅ |
| `voteFact` | writes.spec | ✅ |
| `submitAiClaim` | writes.spec | ✅ |
| `finalizeProposals` | writes.spec (finalize) | ✅ |
| `advancePhase` | cranks.spec | ✅ |
| `finalizeFacts` | cranks.spec | ✅ |
| `finalizeAiClaims` | cranks.spec | ✅ |
| `finalizeOracle` | settle.spec | ✅ |
| `claimProposer` | writes.spec (claim) | ✅ |
| `claimFact` | terminal.spec | ✅ |
| `claimFactVote` | terminal.spec | ✅ |
| `closeAiClaim` | terminal.spec | ✅ |
| `sweepOracle` | sweep.spec | ✅ |
| `setGovernance` | admin.spec | ✅ |
| `setConfig` | admin.spec | ✅ |
| `resolveDeadend` | admin.spec | ✅ |
| `kassPrice` | admin.spec | ✅ |

The four DAO ops had **no UI** — the `/admin` page + `data/actions/admin.ts` were
built for them. Each is gated on-chain by `Protocol.admin`/`dao_authority`; a real
hand-off routes through a Squads vault PDA no test keypair can sign, so the spec
fabricates the exact governance field per-test via `patchProtocol`.

**Forked run** (`scripts/e2e-playwright-fork.sh`) — the challenge-market cluster,
one serial flow over a Challenge-phase oracle:

| Instruction | Driven via | Status |
|---|---|---|
| `openChallenge` | `ChallengeComposeForm` (7-step client-side compose→open) | ✅ |
| `swap` | `ChallengeTradeControls` · SwapForm (a real fail-pool buy) | ✅ |
| `crankTwap` | `ChallengeTradeControls` · CrankForm (pass + fail pools) | ✅ |
| `settleChallenge` | `ChallengeTradeControls` · one-click derive-from-Market settle | ✅ |
| `closeMarket` | `CloseControl` (after `finalizeOracle` → Resolved) | ✅ |

**Indexer run** (`scripts/e2e-playwright-indexer.sh`) — the whole indexing
pipeline end to end. `e2e/indexer/global-setup.ts` boots surfpool, seeds an
oracle with real transactions (create_oracle → propose×2 → finalize_proposals →
submit_fact), then runs the **actual `kassandra-indexer` binary** (the one
deployed on Render) against surfpool's RPC + an **ephemeral Postgres**
(`e2e/indexer/pg.ts` — `initdb`/`pg_ctl` in a temp dir). Once it has crawled the
activity, the app is loaded with `VITE_INDEXER_URL` pointed at it and the spec
asserts the on-chain **ActivityFeed** renders those instructions (chain → Carbon
crawler → Postgres → read API → app). Needs the indexer binary built + the
postgres binaries on PATH (`PG_BIN` overrides).

The compose form now takes the **challenged proposer** (query-param default) so
`open_challenge` derives the right `ai_claim`/`market` — previously it wrongly
passed the connected wallet. `fork/onchain.ts` backdates `Market.twap_end` /
`Oracle.phase_ends_at` so the settle + finalize gates open without waiting the
real TWAP / challenge windows.

## Adding a spec

1. Seed the required phase in `global-setup.ts` (reuse `seed.ts`; call
   `keepWindowOpen` if the action needs an open window at test time).
2. Add a `*.spec.ts` that injects the wallet, opens the oracle, performs the UI
   action, and polls the chain (`onchain.ts`) for the effect.
