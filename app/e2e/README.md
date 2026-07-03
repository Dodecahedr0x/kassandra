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

## Files

- `global-setup.ts` — boot surfpool + seed one oracle per action.
- `seed.ts` — reusable phase drivers (create → dispute → FactProposal → FactVoting
  → AiClaim), + `keepWindowOpen` (patches `phase_ends_at` to the far future, since
  surfpool time-travel is forward-only and a window closed by later seeding can't
  be re-entered by rewinding the clock).
- `onchain.ts` — read/decode accounts + a forward clock set.
- `e2eWallet.tsx` (in `src/lib`) — the real-signing wallet.
- `*.spec.ts` — the tests.

## Coverage

Every action is driven through the real app UI and asserted on-chain.

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
| `claimProposer` | writes.spec (claim) | ✅ |

### Remaining (harness ready; each is an added seed + spec)

- **`finalizeOracle`** — needs a Challenge-phase oracle with surviving proposers
  (drive to AiClaim, submit AI claims for the proposers, `finalizeAiClaims` →
  Challenge, then crank `finalizeOracle`).
- **`claimFact` / `claimFactVote` / `closeAiClaim`** — need a disputed oracle
  driven to a terminal state with the **wallet** as the fact submitter / voter, so
  the connected wallet has something to claim/close.
- **`sweepOracle` / `closeMarket`** — need governance fabricated (patch
  `Protocol.dao_authority`) + the 30-day sweep grace elapsed / a settled Market.
- **Challenge market cluster** (`openChallenge`, `swap`, `crankTwap`,
  `settleChallenge`) — needs surfpool **forking mainnet** so MetaDAO's deployed
  conditional-vault + AMM programs are executable (network-dependent + slower; the
  same setup the gated vitest `challenge.e2e.test.ts` uses). Run these in a
  separate, network-gated Playwright project.
- **Admin/DAO** (`setConfig`, `setGovernance`, `resolveDeadend`, `kassPrice`) — not
  participant flows; require the DAO authority (governance fabrication) and are
  covered by the program's LiteSVM tests.

## Adding a spec

1. Seed the required phase in `global-setup.ts` (reuse `seed.ts`; call
   `keepWindowOpen` if the action needs an open window at test time).
2. Add a `*.spec.ts` that injects the wallet, opens the oracle, performs the UI
   action, and polls the chain (`onchain.ts`) for the effect.
