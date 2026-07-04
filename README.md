# Kassandra

**A decentralized, AI-assisted optimistic oracle on Solana.**

Kassandra answers **binary and categorical** questions. The common case is cheap: an
uncontested proposal settles with no AI and no markets. The dispute machinery — fact
agreement, AI claims, and decision markets — only fires when proposers disagree.

The core idea: **interpretation is fixed at oracle creation**, so disputes reduce to *which
evidence is real and relevant* (objective) rather than *what the evidence means*
(subjective). An AI applies that fixed interpretation to an agreed fact set, and a
MetaDAO-style decision market is the ultimate arbiter that can override a faulty AI claim.

No zkTLS, no TEEs. Honesty is enforced **economically** (KASS staking and slashing) and by
**markets** (the final arbiter of truth).

> **Full documentation** lives in [`docs-site/`](./docs-site) — an extensive Mintlify site
> (concepts, architecture, protocol reference, challenge markets, SDK, and dApp guide). See
> [`docs/plans/2026-06-29-kassandra-design.md`](./docs/plans/2026-06-29-kassandra-design.md)
> for the original design document.

## How an oracle resolves

1. **Create** — a creator posts a prompt, immutable interpretation rules, categorical
   options, and a deadline, and pays a dynamic KASS creation fee (burned).
2. **Propose** — after the deadline, proposers submit a categorical value plus a KASS bond,
   no proofs. If everyone agrees, the oracle **resolves** immediately — no AI, no markets.
3. **Dispute** (only on conflict) — two or more distinct values lock the proposers in and
   open a **fact proposal** window, then a disjoint **fact voting** window that freezes the
   agreed evidence set.
4. **AI claims** — each locked-in proposer reruns the open-source runner over the agreed
   facts and resubmits a value plus AI-claim metadata (model, params, hashes).
5. **Challenge** — every AI claim is challengeable in parallel; a challenge opens a MetaDAO
   decision market, and a fail-vs-pass TWAP decides whether the claim is disqualified.
6. **Resolve or dead-end** — after the last market settles, the final plurality over
   surviving proposers is computed. If nothing survives (or a tie), the oracle reaches an
   **Invalid dead-end**, resolvable only by KASS governance.

## Monorepo layout

| Path | What it is |
| --- | --- |
| [`programs/kassandra/`](./programs/kassandra) | The core Solana program, written in **Pinocchio** (not Anchor). Owns oracle state, phases, facts, AI claims, plurality, staking, emissions, and the dynamic fee. Program ID `KassVxvXUEPr5apSr2MqiGva4VFtJXyYLLDFS3f83nY`. |
| [`runner/`](./runner) | The open-source AI runner (`kassandra-runner`). Applies the fixed interpretation to the agreed facts and produces a categorical answer plus verifiable metadata. |
| [`sdk/`](./sdk) | A hand-written TypeScript client (`@kassandra/sdk`) — instruction builders, account decoders, and PDA helpers. No IDL; layouts mirror the program. |
| [`app/`](./app) | The frontend (Vite + React) for creating oracles, proposing, fact voting, and trading challenge markets. |
| [`docs-site/`](./docs-site) | The Mintlify documentation site (published via GitHub Actions → GitHub Pages). |
| [`docs/`](./docs) | Design document + the dated implementation plans (`docs/plans/`). |
| [`scripts/`](./scripts) | Helper scripts — dumping MetaDAO program binaries into the test fixtures. |

MetaDAO's deployed **conditional-vault + AMM** programs are reused for the pass/fail
decision markets via CPI; Kassandra does not reimplement the vault or AMM.

## Getting started

### Prerequisites

- **Rust** (stable — see [`rust-toolchain.toml`](./rust-toolchain.toml)) with the
  Solana toolchain (`cargo build-sbf`, from the Solana CLI / Agave).
- **Node.js** and **pnpm** (the `sdk` and `app` form a pnpm workspace).
- [`just`](https://github.com/casey/just) for the program build/test recipes.
- For the e2e / dev-stack targets: [`surfpool`](https://surfpool.run) and Postgres
  (`initdb`/`pg_ctl`).

### One entrypoint: `make`

Every useful task is a `make` target — `make help` lists them all. It delegates to
`cargo`, `just`, `pnpm`, and the `scripts/*.sh`, so there's a single surface:

```bash
make setup       # install JS deps + build the program (.so) and SDK (first run)
make build       # build everything (program, sdk, app, runner, indexer)
make test        # all unit tests (rust workspace + sdk + app + indexer)
make lint        # oxlint (app) + clippy (rust)
make typecheck   # sdk + app tsc
make dev         # boot a seeded local surfpool chain AND the app dev server

make test-e2e            # browser E2E (surfpool + funded wallet + app)
make test-e2e-fork       # mainnet-forked challenge-market E2E
make test-e2e-indexer    # surfpool + Postgres + indexer + ActivityFeed E2E
make ci                  # exactly what CI runs
```

`make dev` (or `make chain` alone) boots surfpool, deploys the program, and seeds a
spread of oracles across phases — then holds the chain alive so you can browse it in
the app. Ctrl-C tears it down.

### Build & test the program

```bash
just build            # cargo build-sbf --manifest-path programs/kassandra/Cargo.toml
just test             # rebuilds the .so first, then runs the LiteSVM test suite
```

The tests are **LiteSVM** unit + invariant + CPI-integration tests. `just test` depends on
`just build` so you never test a stale `.so`.

### Build the SDK and run the dApp

```bash
pnpm install
pnpm --filter sdk build     # the app imports the built SDK
pnpm --filter app dev       # serve the frontend locally
```

See each package's README for details:
[program](./programs/kassandra/README.md) ·
[runner](./runner/README.md) ·
[sdk](./sdk/README.md) ·
[app](./app/README.md) ·
[docs-site](./docs-site/README.md) ·
[scripts](./scripts/README.md).

## Architecture notes

- **Pinocchio, not Anchor.** Manual account deserialization/validation and manual
  instruction dispatch (no macros/IDL). CPI into MetaDAO's Anchor programs is constructed
  by hand (8-byte sighash discriminators + account metas + Borsh args). The trade-off: more
  manual serialization in exchange for a smaller, cheaper, dependency-light program.
- **On-chain:** request config, all stakes/bonds (KASS) and market collateral (USDC), the
  fact set & approvals, AI-claim metadata, plurality result, market triggers, emissions,
  and dynamic-fee state.
- **Off-chain:** model inference, private to each runner. No raw AI output on-chain — only
  the categorical claim and verifiable metadata.
- **Trust model:** economic + market-based. KASS slashing for bad facts/claims; MetaDAO
  decision markets as the ultimate arbiter over a faulty AI claim.

## Tokens

- **KASS** — the SPL token for staking, slashing, and decision-market collateral. No
  presale; fair-launch via participation emissions. Required to propose, to stake on facts,
  and (as a proposer) it is your conditional-market collateral.
- **USDC** — a challenger's stake when opening a decision market.

## Status

Kassandra is under active development. The program, SDK, runner, and dApp are implemented
and covered by LiteSVM and end-to-end (surfpool) tests; economic parameters (emission
curve, fee-EMA constants, reward splits) are still being tuned. See `docs/plans/` for the
implementation history and open items.
