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

Every action is driven through the real app UI and asserted on-chain. The default
run covers the **entire non-challenge protocol surface — 15 instructions**:

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

### Remaining

- **Challenge market cluster** (`openChallenge`, `swap`, `crankTwap`,
  `settleChallenge`, `closeMarket`) — requires surfpool **forking mainnet** so
  MetaDAO's deployed conditional-vault + AMM programs are executable, plus the
  multi-step market composition (question + 2 conditional vaults + split + 2 AMMs
  + liquidity) and the slot-timed TWAP crank. The fork works in this environment
  (the gated vitest `challenge.e2e.test.ts` passes), so this belongs in a
  **separate, network-gated Playwright project** with a forked (`fork: "mainnet"`,
  `blockProductionMode: "clock"`) globalSetup — not the fast default simnet run.
  Its flow is already proven at the app-builder level by that vitest suite.
- **Admin/DAO** (`setConfig`, `setGovernance`, `resolveDeadend`, `kassPrice`) — the
  app ships **no UI** for these; per "create it if missing" they'd need admin
  pages + `build*Ixs` wrappers, and (except `kassPrice`, a read) the DAO authority.
  They are covered by the program's LiteSVM tests today.

## Adding a spec

1. Seed the required phase in `global-setup.ts` (reuse `seed.ts`; call
   `keepWindowOpen` if the action needs an open window at test time).
2. Add a `*.spec.ts` that injects the wallet, opens the oracle, performs the UI
   action, and polls the chain (`onchain.ts`) for the effect.
