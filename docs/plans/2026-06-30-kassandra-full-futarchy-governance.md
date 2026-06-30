# Full Futarchy Governance — Design + Plan

> **For Claude:** REQUIRED SUB-SKILL: subagent-driven-development (per-task implement + review).

**Goal:** Make Kassandra genuinely governed by a live MetaDAO **futarchy** KASS DAO end-to-end: stand up the real DAO + Squads v4 multisig, harden the on-chain `set_governance` handoff to validate the real linkage, build the SDK builders + bootstrap script, and PROVE the full loop on forked-mainnet surfpool — a real futarchy **proposal → conditional pass/fail KASS markets → real TWAP verdict → Squads `vault_transaction_execute` → a Kassandra `set_config`/`resolve_deadend` actually applied on-chain**. Today this is all simulated with fabricated `Dao` blobs + discriminator-dispatch probes; this milestone makes it real.

**Decisions (locked with the user):**
1. **Bootstrap = off-chain SDK script** (create the futarchy `Dao` via `initialize_dao` → the Squads v4 multisig with `create_key==Dao PDA` → call the existing `set_governance` handoff recording the real Squads **vault** PDA + `Dao` account). No program-driven DAO creation (the on-chain `initialize_dao` Borsh stub stays unused).
2. **Verdict = FULLY REAL** — real conditional markets (v0.6 `conditional_vault` + Meteora DAMM v2), seed liquidity, real `conditional_swap` to a pass, crank/await the real TWAP windows, `finalize_proposal`. NO deterministic shortcut for the verdict. (T4 found live AMM-TWAP cranking non-deterministic on a fork → this is the high-risk part: ATTEMPT IT GENUINELY; if a specific step proves intractable on the fork, STOP-and-report the exact blocker — fall back to a documented deferral ONLY after a real attempt, never fake a pass.)
3. **On-chain hardening = YES** — `set_governance` must validate that `kass_dao` is a real futarchy `Dao` (owner==`FUTARCHY_ID` + discriminator) and that `dao_authority` is the Squads **vault** PDA derived for that DAO — so a bogus/mismatched handoff is rejected on-chain.

**Architecture / source of truth.** The on-chain governance surface (the four instructions `set_governance`/`set_config`/`resolve_deadend`/`kass_price` + their gating + the CPI scaffolding in `cpi/metadao_v06.rs`: program IDs, Anchor discriminators, PDA seed builders/derivers, the `futarchy_spot_twap` reader, invoke wrappers) is ALREADY built + seam-tested. This milestone adds: the G1 program hardening; the SDK futarchy/Squads builders + bootstrap (G2); the fully-real surfpool E2E (G3); docs (G4). Builds on the merged surfpool harness (`sdk/test/surfpool/` — fork mode, `surfnet_timeTravel`, `surfnet_cloneProgramAccount`) + the SDK. The program is otherwise read-only (only `set_governance` + its tests change in G1); the runner is untouched.

**Tech Stack:** Rust (program: Pinocchio; `cpi/metadao_v06.rs` has the Squads/futarchy seed derivers G1 reuses); the SDK (TS, web3.js v3); surfpool 1.0.0 (fork mode for MetaDAO); the existing gated E2E infra (`KASSANDRA_E2E=1`). MetaDAO program IDs: futarchy `FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq`, conditional_vault `VLTX1ish…`, Meteora DAMM v2 `cpamd…`, Squads v4 `SQDS4…`.

## Tasks

### G1 — Harden the on-chain `set_governance` handoff (program)
- READ `programs/kassandra/src/processor/set_governance.rs` (current: stores payload pubkeys verbatim while `governance_set==0`, admin-gated) + `cpi/metadao_v06.rs` (the Squads multisig/vault PDA seed builders/derivers + `FUTARCHY_ID` + the `Dao` account layout/discriminator) + `state.rs` (Protocol `dao_authority`/`kass_dao`/`governance_set`) + `tests/governance_seam.rs`.
- Change `set_governance` to VALIDATE the linkage (not just store):
  - The instruction now takes the **`kass_dao` account** (read-only) in addition to protocol + authority. Assert `kass_dao.owner == FUTARCHY_ID` and the `Dao` Anchor discriminator matches (reject otherwise → a clear new `KassandraError`).
  - Assert the payload `dao_authority` equals the **Squads v4 vault PDA derived for this DAO** (derive in-program via the `cpi/metadao_v06.rs` Squads seed deriver — the multisig `create_key == Dao PDA` per the design; derive multisig → vault and compare). Reject mismatch.
  - Keep the existing one-shot handoff semantics (admin only while `governance_set==0`; then only the current `dao_authority`; never back to admin) + the non-zero checks. Don't change account sizes (no state-layout size change; if you add fields you must re-pin — prefer NOT to).
- Add the new error code(s) (append to `KassandraError`, ABI-stable). Update the SDK's `setGovernance` builder (`sdk/src/instructions/lifecycle.ts`) to add the `kass_dao` account in the right slot/role + the SDK parity test if it pins set_governance. Extend `tests/governance_seam.rs` (+ any set_governance test): accept the real derived-vault + real Dao; reject a wrong vault, a non-futarchy-owned kass_dao, a bad discriminator.
- `just build` + `cargo test -p kassandra-program` (all green; new + existing) + clippy + fmt; `cd sdk && pnpm typecheck && pnpm test` (the setGovernance account change must keep the SDK suite green incl. parity). Commit `feat(governance): set_governance validates real Squads-vault/futarchy-DAO linkage`.

### G2 — SDK futarchy/Squads builders + the off-chain bootstrap script (recon-heavy; make-or-break for G3)
- RECON (write `sdk/src/futarchy/NOTES.md`): the exact instruction layouts (discriminators + arg encodings + account metas) for the futarchy v0.6 + Squads v4 + (as needed) Meteora DAMM v2 + conditional_vault instructions the lifecycle needs — `initialize_dao`, `initialize_proposal`, `launch_proposal`, `finalize_proposal`, `conditional_swap` (+ the spot-oracle crank/observe if separate); Squads `multisig_create`(v4)/`vault_transaction_create`/`vault_transaction_execute`; conditional_vault `initialize_question`/`initialize_conditional_vault`/`split_tokens`/`merge`/`redeem`; Meteora pool init/add-liquidity/swap. Mirror the Rust `cpi/metadao_v06.rs` discriminators/seed derivers (the authoritative, binary-validated source) and reverse-engineer the arg layouts from the MetaDAO IDLs / dumped binaries (the T4 challenge-market test + `tests/challenge_e2e.rs` already compose some of these — REUSE their account/arg composition). If a layout can't be authoritatively determined, STOP-and-report (don't guess wire formats).
- Build SDK builders (TS, web3.js v3) for those instructions under `sdk/src/futarchy/` (exported), + PDA derivers matching the Rust ones (Dao, Proposal, Squads multisig/vault, conditional vault/question, event authorities). Unit-test the byte layout of each builder (disc + args + metas) against the known discriminators/seeds where possible.
- The **bootstrap script** (`sdk/src/futarchy/bootstrap.ts` or a script): given a connection + payer + the KASS/USDC mints, create the futarchy `Dao` (`initialize_dao`) + the Squads multisig (`create_key==Dao PDA`) + derive the vault, then call the (G1-hardened) `setGovernance` handoff with `dao_authority=vault`, `kass_dao=Dao`. Returns the created addresses. (Unit/typecheck here; the live run is G3.)
- `cd sdk && pnpm typecheck && pnpm test` green (default offline). Commit `feat(sdk): futarchy v0.6 + Squads v4 instruction builders + governance bootstrap`.

### G3 — surfpool E2E: the FULL futarchy governance loop, fully real (headline; high-risk)
- A gated (`KASSANDRA_E2E=1`) surfpool test on **forked mainnet** (the T4 fork harness): 
  1. **Bootstrap (real):** deploy Kassandra + init_protocol, create the KASS/USDC mints, run the G2 bootstrap (real `initialize_dao` + Squads multisig + the G1-hardened `set_governance` handoff). Assert `Protocol.governance_set==1`, `dao_authority`==the real vault, `kass_dao`==the real Dao.
  2. **Proposal (real):** build a futarchy proposal whose action is a Kassandra `set_config` (change a governable param to a new value) staged as a Squads `VaultTransaction`; `initialize_proposal` → `launch_proposal` standing up the real conditional pass/fail KASS markets (v0.6 conditional_vault + Meteora DAMM v2).
  3. **Verdict (FULLY REAL):** seed liquidity into the pass/fail markets; real `conditional_swap`(s) to push the **pass** TWAP above fail; crank/await the real TWAP windows (use `surfnet_timeTravel` to cross the ≥150-slot/delayed-twap windows; the cp-amm/Meteora observation must accumulate — this is the part T4 found hard, attempt genuinely); `finalize_proposal` → assert it marks **Passed**.
  4. **Execute (real):** `vault_transaction_execute` via the real Squads vault → it `invoke_signed`s the staged `set_config` CPI into Kassandra → fetch + `decodeProtocol` → assert the governable param CHANGED to the proposed value. This is the headline: a real futarchy verdict drove a real Kassandra config change through Squads.
  5. **Second arm:** a `resolve_deadend`-via-governance proposal on a seeded `InvalidDeadend` oracle → executed via the same path → assert `Resolved`. AND the **live `kass_price`** read: read the futarchy spot TWAP from the REAL `Dao` (replacing the fabricated `buildDaoBlob`) and assert `kass_price` returns a sane value.
- GENUINE attempt; if a specific step (e.g. the TWAP crank / market observation) proves intractable on the fork after a real attempt, STOP-and-report the exact blocker (the program error / the missing crank) so we decide — then it may be documented as a deferral, NEVER faked. Keep the default `pnpm test` 72 offline; the gated suite spawns surfpool.
- Commit `test(e2e): full futarchy governance loop on forked MetaDAO (proposal->TWAP->Squads->set_config)`.

### G4 — docs + covered-vs-deferred
- A README/section for the futarchy governance E2E: prerequisites, how to run, and WHAT'S PROVEN (the real loop, or the exact extent reached) vs DEFERRED (anything the fully-real attempt couldn't drive on the fork, with the precise reason). Update `sdk/test/surfpool/README.md` + append the final note to this plan. Commit `docs(governance): full-futarchy-governance E2E coverage + deferrals`.

## Out of scope / deferred
- Program-driven DAO creation (finishing the on-chain `initialize_dao` Borsh stub) — bootstrap is off-chain by decision.
- Dead-end economic settlement (token movement for a governance-resolved dead-end) — belongs to the settlement milestone; `resolve_deadend` only stamps the outcome.
- Live-cluster/devnet governance with real funds; mainnet deployment of the real KASS DAO.

## Delta log

### G1 — `set_governance` validates the real Squads-vault / futarchy-DAO linkage (DONE)
- **Program (`processor/set_governance.rs`):** added the `kass_dao` account as a
  third, read-only account (`[protocol(w), authority(signer), kass_dao(ro)]`).
  After the existing non-zero + one-shot/auth gates, the handoff now VALIDATES:
  (a) `kass_dao_ai.key() == payload.kass_dao` (`assert_key` → `InvalidAccount`);
  (b) `kass_dao_ai.is_owned_by(FUTARCHY_ID)` AND first 8 bytes ==
  `DAO_ACCOUNT_DISCRIMINATOR` (→ `InvalidFutarchyDao`); (c) `payload.dao_authority
  == squads_vault_pda(squads_multisig_pda(kass_dao), 0)` derived in-program via
  the `cpi/metadao_v06.rs` Squads derivers (multisig `create_key == kass_dao` →
  multisig → vault idx 0) (→ `DaoAuthorityMismatch`). One-shot / rotation / never-
  back-to-admin semantics + the `governance_set=1` stamp PRESERVED; no `Protocol`
  size/layout change. Derivation confirmed identical to the test harness's
  recorded vault (the old `governance_seam::derive_squads_vault`, now
  `TestCtx::squads_vault_for_dao`).
- **Errors:** appended `InvalidFutarchyDao = 31`, `DaoAuthorityMismatch = 32`
  (ABI-stable; existing 0..=30 unchanged). Mirrored in `sdk/src/constants.ts`
  (enum + messages) + `sdk/test/parity.test.ts` (PINNED map + count 33).
- **SDK (`instructions/lifecycle.ts`):** `setGovernance` now appends the
  `kass_dao` read-only account after the authority; payload unchanged. Lifecycle
  byte-parity test updated for the 3rd meta.
- **Tests:** `governance_setup.rs` — accept-real (fabricated futarchy `Dao` +
  derived vault), one-shot admin-rejected, + 3 reject arms (non-futarchy owner,
  bad discriminator, wrong vault). `governance_seam.rs` gate tests rewired to the
  real validated handoff (accept-real) + a direct-write path. New harness helpers
  in `tests/common/mod.rs`: `squads_vault_for_dao`, `fabricate_dao_and_vault`,
  and `force_governance` (direct Protocol write, used by `set_config` /
  `resolve_deadend` / emissions / kass_price gate-setups + `bless_kass_price`,
  which need a SIGNABLE `dao_authority` the hardened handoff can never produce).
- **Verification:** `just build` ok; `cargo test -p kassandra-program` all green
  (incl. state_layout, governance_setup, governance_seam, set_config,
  resolve_deadend, kass_price); `cargo clippy` + `cargo fmt` clean; `cd sdk &&
  pnpm typecheck && pnpm test` → 72 offline tests green.

### G2 — SDK futarchy/Squads builders + governance bootstrap (DONE)
- **CRITICAL `create_key == Dao` gate — CONFIRMED (authoritative source).** Fetched
  `metaDAOproject/futarchy@v0.6.0 programs/futarchy/src/instructions/initialize_dao.rs`
  (via `gh api`, cross-checked against the dumped `metadao_futarchy_v06.so`
  account-name strings). `initialize_dao` ITSELF creates the Squads multisig via
  CPI `multisig_create_v2` with `create_key: dao.to_account_info()` (the Dao PDA),
  `config_authority = dao`, `threshold = 1`, members `[dao(Vote|Execute),
  permissionless EP3SoC2…(Initiate|Execute)]`; multisig seeds `[b"multisig",
  b"multisig", dao]`, vault index 0. **⇒ G1's hardened check is CORRECT** (no
  revision needed). Deviation handled: the multisig is NOT created as a separate
  bootstrap step (the plan's literal wording); `initialize_dao` creates it
  atomically, so the bootstrap = `initialize_dao` → derive multisig/vault →
  `set_governance`. (A standalone `multisigCreateV2` disc is still exported but
  unused.)
- **RECON → `sdk/src/futarchy/NOTES.md`:** authoritative layout map. conditional_vault
  (init_question/init_vault/split/merge/redeem/resolve_question) = FULLY validated
  against the real binaries (existing Rust + T4 tests). futarchy
  (initialize_dao/initialize_proposal/launch_proposal/finalize_proposal/
  conditional_swap/spot_swap) account orders + args lifted from the v0.6.0 source.
  Squads (vault_transaction_create/execute, proposal_create) from
  `Squads-Protocol/v4@6d5235da`. **STOP-REPORTED:** Meteora DAMM v2 pool/
  add-liquidity/swap builders NOT built — the v0.6 conditional VERDICT markets are
  the futarchy program's EMBEDDED AMM (`Dao.amm`, driven by launch/conditional_swap/
  finalize), NOT Meteora; Meteora is only spot-liquidity/fee-collection and its
  zero-copy `Pool` offsets (`sqrt_price`) are the flagged deferred unknown (pin at
  G3 if exercised).
- **Builders** under `sdk/src/futarchy/` (exported as `futarchy.*`):
  `constants.ts` (discriminators/seeds/`Market`/`SwapType`), `pda.ts` (Dao,
  Proposal, Squads multisig/vault/program_config/spending_limit/transaction/
  proposal, conditional vault/question/cond-token mint, both event authorities),
  `instructions.ts` (the futarchy + conditional_vault + Squads builders, web3.js v3
  `TransactionInstruction`s, event_cpi tail appended), `bootstrap.ts`
  (`bootstrapGovernance` → `{dao, multisig, vault, programConfig, spendingLimit,
  instructions:[initialize_dao, set_governance]}`).
- **Tests:** `sdk/test/futarchy.test.ts` (16 tests) pins each builder's
  `data == [disc, ...borsh_args]` + the account-meta order/roles (independently
  re-derived PDAs), the Market/SwapType tags, and the bootstrap sequence
  (`vault == dao_authority`, `kass_dao == dao`).
- **Verification:** `cd sdk && pnpm typecheck` clean, `pnpm build` clean,
  `pnpm test` → 88 offline (72 prior + 16 new) green. Program + runner untouched.

### G3 — FULL futarchy governance loop on forked MetaDAO, FULLY REAL (DONE — headline proven)
- **PROVEN end-to-end on forked mainnet** (`sdk/test/surfpool/futarchy-governance-e2e.test.ts`,
  gated `KASSANDRA_E2E=1`, 2 arms, both green). A REAL futarchy TWAP verdict drove
  a REAL Kassandra `set_config` through Squads — no fake, no deterministic
  shortcut for the verdict.
- **Arm 1 — bootstrap (real):** `bootstrapGovernance` → real `initialize_dao`
  (creates the `Dao` + the Squads v4 multisig with `create_key==Dao` + vault
  atomically; `program_config.treasury` fetched LIVE from the on-chain
  ProgramConfig @ offset 48) → the G1-hardened `set_governance`. Asserts
  on-chain `governanceSet==1`, `daoAuthority==vault`, `kassDao==dao` (G1's
  hardened linkage check validated against the REAL Squads vault / futarchy DAO).
- **Arm 2 — stage → proposal → launch → verdict → execute:**
  1. `provide_liquidity` seeds the embedded spot AMM (price = 1e12 = PRICE_SCALE).
  2. Stages a Kassandra `set_config` (sentinel `total_supply_cap`) **AND** a
     `resolve_deadend` (on a fabricated `InvalidDeadend` oracle) as TWO inner CPIs
     in ONE Squads `VaultTransaction` (compact `TransactionMessage` hand-encoded:
     vault = ro-signer, protocol+oracle = w-non-signers, Kassandra prog = ro
     program; `data` u16-prefixed) + `proposal_create(draft:false → Active)`,
     signed by the public permissionless member (`EP3SoC2…`).
  3. `initialize_question` (oracle == futarchy Proposal PDA) + base/quote
     `initialize_conditional_vault` → `initialize_proposal` → `launch_proposal`.
  4. **FULLY-REAL verdict:** trader splits USDC → conditional pass/fail quote
     tokens, then 4× `conditional_swap` Buy-Pass (>60s apart via `surfnet_timeTravel`,
     the oracle's 60s rate-limit) to raise the pass observation, then jumps past
     `enqueue + 86400` and a final swap stamps the oracle beyond the
     ProposalTooYoung / MarketsTooYoung windows. `finalize_proposal` resolves
     **Passed** (`Proposal.state` tag == 2) → CPIs Squads `proposal_approve`.
  5. `vault_transaction_execute` (member = permissionless) `invoke_signed`s BOTH
     CPIs as the vault → **HEADLINE asserts `Protocol.total_supply_cap` == the
     sentinel** (a real TWAP verdict drove a real config change via Squads), AND
     the dead-ended oracle → `Phase::Resolved` + `resolved_option`.
  6. **Live `kass_price`:** reads the futarchy spot TWAP from the REAL `Dao`
     (not `buildDaoBlob`) via a simulated `kass_price` tx's return data → > 0.
- **DEPLOYED == v0.6.1, not v0.6.0 (recon correction).** The on-chain program
  (fetched via its Anchor IDL) is v0.6.1. Three SDK builders were corrected
  (program + runner UNTOUCHED): `initialize_dao` args +`team_sponsored_pass_
  threshold_bps:i16`+`team_address:Pubkey` (data 83→117 B); `initialize_proposal`
  +`squads_multisig` (12 accts); `launch_proposal` +`squads_multisig`+`squads_
  proposal` (20 accts). Added a `provide_liquidity` builder + `AmmPosition` PDA.
  Parity test (`futarchy.test.ts`) pins updated; `sdk/src/futarchy/NOTES.md` has
  the v0.6.1 addendum + the Squads `TransactionMessage` wire spec.
- **Nothing deferred / no fake.** The full loop ran genuinely; the only T4-flagged
  risk (live AMM-TWAP cranking) was satisfied by the 60s-spaced swaps + the
  one-day `timeTravel` + final swap. Default `pnpm test` stays 88 offline
  (surfpool suite excluded by `vitest.config.ts`); `cd sdk && pnpm typecheck`
  clean; `just build` green; program/runner read-only.

## Execution note
After each task: build/test green; the default `pnpm test` stays 72 offline; `cargo test -p kassandra-program` green. G1 is a focused, reviewed PROGRAM change (the only program edit). G2 is the make-or-break SDK builder work (stop-and-report if a MetaDAO wire format can't be authoritatively determined). G3 is the fully-real headline (genuine attempt; stop-and-report a real blocker rather than fake). Append a G1–G4 delta log here.
