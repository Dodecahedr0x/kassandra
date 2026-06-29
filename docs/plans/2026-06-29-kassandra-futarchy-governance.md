# Kassandra KASS Futarchy Governance — Design + Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the Kassandra protocol **governed by a MetaDAO v0.6 futarchy KASS DAO**. Governance can (a) update protocol config and (b) resolve `InvalidDeadend` oracles. The DAO's KASS/USDC spot market (Meteora DAMM v2) exposes a **TWAP price** (`kass_price`) that the *next* milestone (challenge-market rework) will consume. This is the first step of the dependency-first roadmap: **KASS futarchy → challenge-market rework → staker settlement** (see `2026-06-29-kassandra-settlement-economics.md`).

**Architecture:** Extends the existing Pinocchio program (no Anchor). Reuses **MetaDAO futarchy v0.6** (its governance program + v0.6 conditional vault + **Meteora DAMM v2** AMM) via dumped fixtures + hand-built CPI / LiteSVM, the same way the dispute core reused the v0.4 vault/AMM. NOTE: v0.6 is a **separate, newer stack** than the dispute core's pinned v0.4 vault/AMM (v0.5+ migrated the AMM to Meteora DAMM v2) — this milestone integrates that newer stack.

**Tech Stack:** Rust, `pinocchio` 0.8, `bytemuck`, `litesvm`, `solana-sdk` (test-only), `spl-token`, MetaDAO futarchy **v0.6** + v0.6 conditional vault + **Meteora DAMM v2**.

**Source of truth:** design `docs/plans/2026-06-29-kassandra-design.md`; the dispute-core deltas in `2026-06-29-kassandra-dispute-core.md` ("Implementation deltas (live state)" — authoritative live types/sizes/guards/seeds/errors/Ix); the happy-path milestone (now merged: `init_protocol`/`Protocol`, `create_oracle`, `propose`, `finalize_proposals`, EMA fee, emission consts pending settlement). FOLLOW THE LIVE STATE.

---

## Validated design (brainstormed)

### Governance seam
- `Protocol` gains `dao_authority: Pubkey` (the v0.6 DAO execution-authority PDA) and `kass_usdc_pool: Pubkey` (the canonical Meteora DAMM v2 KASS/USDC pool). Two privileged instructions, each **gated to require `dao_authority` as signer**: `set_config`, `resolve_deadend`. A *passed* v0.6 proposal CPIs into them — no privileged key; governance-by-market end to end.

### Governable config — snapshot-at-creation
- Governable params live **mutable on `Protocol`** (edited by `set_config`) and are **snapshotted onto each `Oracle` at `create_oracle`**. Downstream processors read them from the `Oracle` they already load (no new account threading). New oracles pick up new config; in-flight oracles keep their snapshot (a mid-dispute governance change cannot move the goalposts).
  - **Snapshot onto `Oracle`** (per-oracle behavioral): `THRESHOLD_NUM/DEN`, `MARKET_THRESHOLD_NUM/DEN`, `FLIP_SLASH_NUM/DEN`, `FACT_VOTE_SLASH_NUM/DEN` (settlement-era; reserve the field now if cheap), reward-bucket weights `PW/FW` (settlement-era; reserve), window durations `PHASE_WINDOW`/`PROPOSAL_WINDOW`. (`twap_window` already per-oracle.)
  - **Global on `Protocol`** (monetary, used by `create_oracle` which loads `Protocol`): fee-EMA params, emission rate, `TOTAL_SUPPLY_CAP`.
  - **Fixed `const` (NOT governable):** `MAX_PROPOSERS` (tx-size/liveness constraint), anything affecting account layout.
- `set_config` updates only the `Protocol`-resident governable fields, bounds-checked (denominators > 0, fractions ≤ 1, windows > 0); never retroactively touches existing oracles.

### Dead-end resolution
- `resolve_deadend(oracle, option)` gated to `dao_authority`: `require_phase(InvalidDeadend)`, `option < options_count` → set `Phase::Resolved` + `resolved_option`. The **economic settlement** of a governance-resolved dead-end is deferred to the settlement milestone (likely stakes returned, no rewards) — this milestone only sets the terminal outcome.

### Price oracle
- `kass_price` reads the **canonical KASS/USDC Meteora DAMM v2 pool TWAP** (layout from F0 recon), asserting the passed pool == `Protocol.kass_usdc_pool` (governance-blessed; prevents attacker-pool substitution). Ships as a validated primitive with **no on-chain consumer yet** (the challenge-market rework consumes it next milestone) — expected, not dead code.

### Bootstrapping
- An init/setup step records `dao_authority` + `kass_usdc_pool` in `Protocol` and confirms the KASS mint authority is the program PDA. (Emission mint authority stays the program PDA per the settlement design; the DAO governs the emission *rate*, not direct minting.)

---

## Conventions (unchanged)
- TDD; `just build` (cargo build-sbf) BEFORE `cargo test`; clippy + fmt clean before commit. Commit trailer `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`, git author `Kassandra <hexadecifish@gmail.com>`. Append-only `Ix`/`KassandraError` discriminants. Re-pin `tests/state_layout.rs` on any layout change. rust-analyzer false positives — rely on real cargo runs.

## Live-state entry points (post happy-path merge)
- `Protocol` (LEN 128): account_type, admin, kass_mint@40, usdc_mint@72, fee_ema:u64@104, last_creation_unix:i64@112, bump. `load_protocol` (owner+len+tag+ PDA-address pin) exists. PDA `[b"protocol"]`; create-or-adopt init (Allocate+Assign) tolerant of pre-funding.
- `Oracle` (LEN 232) with `resolved_option`@197, `open_challenge_count`@198, phase/windows/counts/`dispute_bond_total`. `Phase{...Resolved=7, InvalidDeadend=8}`.
- `Ix` up to `FinalizeProposals=12`. `KassandraError` up to `NoProposals=24`. Guards: `assert_*`, `load_oracle/fact/proposer/ai_claim/protocol`, `create_pda`. `config.rs` consts incl. `MAX_PROPOSERS=60`, windows, thresholds, fee/flip params.
- Existing MetaDAO v0.4 integration in `src/cpi/metadao.rs` + `tests/fixtures/` (do NOT disturb; v0.6 is additive — consider `src/cpi/metadao_v06.rs` + `tests/fixtures/` additions).

---

## Tasks

### F0 — MetaDAO v0.6 + Meteora DAMM v2 recon + CPI groundwork (HIGH RISK)
Mirror Task 9's rigor. STEP 0: verify mainnet reachability; resolve **authoritative latest** program IDs for the v0.6 futarchy/governance program, the v0.6 conditional vault, and **Meteora DAMM v2** from MetaDAO's official source (`declare_id!` + on-chain), REJECTING web-guessed IDs. If blocked (no mainnet / can't resolve), STOP and report. Then: `scripts/fetch-metadao-v06.sh` dumps the binaries to `tests/fixtures/` (sha-pin in the header). `src/cpi/metadao_v06.rs`: program IDs, discriminators (sha256("global:<name>")[..8]), PDA seeds, no-alloc arg encoders, invoke wrappers for the instructions we need. Document the REAL layouts: the proposal account, the DAO account, the **DAO execution-authority PDA**, and the **Meteora DAMM v2 pool + TWAP** field offsets (determined from real source/binary, NOT guessed). `tests/metadao_v06_cpi.rs`: load all v0.6 + Meteora fixtures into LiteSVM without panic; validate a minimal CPI (e.g. read a Meteora pool's TWAP, or initialize a DAO) against the real binary. Report the resolved IDs/versions/sources, the layouts, and what was validated vs deferred.

### F1 — Protocol governance state + DAO linkage
Add to `Protocol` (re-pin layout): `dao_authority: Pubkey`, `kass_usdc_pool: Pubkey`, and the global governable params (fee-EMA params, emission rate, `TOTAL_SUPPLY_CAP` — reserve fields even if settlement sets their semantics later). Add an init/setup instruction (or extend `init_protocol`) — gated appropriately — that records `dao_authority` + `kass_usdc_pool` and confirms/sets the KASS mint authority = program PDA. Tests: setup records the fields; the canonical pool is pinned. (Bootstrapping note: in v1, who sets `dao_authority` initially — the `admin` from `init_protocol`, transferring to the DAO — document the trust assumption.)

### F2 — Config-as-state refactor (largest churn)
Add the snapshot fields to `Oracle` (re-pin layout). `create_oracle` snapshots the current global governable per-oracle params from `Protocol`/config into the `Oracle`. Switch every processor that reads a snapshotted param from `config::X` to `oracle.x` (finalize_facts, vote_fact, submit_fact, finalize_ai_claims, settle_challenge, finalize_oracle, advance_phase, propose, finalize_proposals — wherever a snapshotted const is used). Keep `MAX_PROPOSERS` + layout sizes `const`. All existing tests must still pass (behavior identical when config == defaults). Re-pin layouts; update the conservation/invariant assumptions only if needed.

### F3 — `set_config` (Ix append; gated)
Gated to `Protocol.dao_authority` (signer). Updates the `Protocol`-resident global governable fields, bounds-checked. Does NOT touch existing oracles. Tests: dao_authority can set; non-authority → Unauthorized; out-of-bounds → error; a subsequently-created oracle snapshots the new values.

### F4 — `resolve_deadend` (Ix append; gated)
Gated to `dao_authority`. `require_phase(InvalidDeadend)`, `option < oracle.options_count` → `set_phase(Resolved)` + `resolved_option`. Document that economic settlement is deferred. Tests: dao_authority resolves a dead-end → Resolved+option; non-authority → Unauthorized; wrong phase → WrongPhase; option out of range → error.

### F5 — `kass_price` (Meteora DAMM v2 TWAP read)
A read (instruction or pure helper over a passed pool account) that returns the KASS/USDC TWAP from the canonical Meteora DAMM v2 pool, asserting `pool == Protocol.kass_usdc_pool`. Layout from F0. Test: a cranked Meteora pool yields a sane TWAP; wrong pool → rejected. No on-chain consumer yet (next milestone).

### F6 — v0.6 futarchy proposal→execute integration (+ seam fallback)
Drive a governance proposal carrying a `set_config` (or `resolve_deadend`) CPI through the v0.6 futarchy: create proposal → conditional pass/fail KASS markets (v0.6 vault + Meteora DAMM v2) → trade to a pass verdict → execute → assert the config changed / oracle resolved, with execution signed by `dao_authority`. **Fallback (document honestly):** if driving the full v0.6+Meteora flow in LiteSVM is impractical, test the **seam** directly (privileged instructions accept the real `dao_authority` PDA as signer and reject others; CPI shapes validated against the dumped binaries) and integration-test the v0.6 execution path as far as LiteSVM allows.

---

## Out of scope (later milestones)
- Challenge-market rework (bond-as-AMM-liquidity + directional fees) consuming `kass_price` — NEXT.
- Staker settlement (returns/rewards/emissions/closure) — see the settlement-economics note.
- Full DAO treasury spending; migrating the dispute-core challenge markets from v0.4 to v0.6/Meteora.

## Execution note
After each task: `just build` → `cargo test -p kassandra-program` → clippy/fmt, confirm green, commit. Never proceed on a red bar. Keep an "Implementation deltas (F0–F6)" running log appended here. F0 is the highest risk (resolve real v0.6/Meteora IDs + layouts first); F2 is the largest churn; F6 is the hardest test (with the documented seam fallback).

---

## Implementation deltas (F0–F6) — live state

### F0 — MetaDAO v0.6 + Meteora DAMM v2 recon + CPI groundwork (DONE 2026-06-29)

**Environment.** solana-cli 3.1.7 (Agave), cargo 1.94.1; mainnet-beta reachable (`solana cluster-version -u m` → 4.0.3). Step 0 UNBLOCKED.

**Resolved program IDs (authoritatively sourced, verified on-chain mainnet-beta 2026-06-29):**

| program | id | version / source |
|---|---|---|
| futarchy (v0.6 governance, replaces `autocrat`) | `FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq` | v0.6.0 — `declare_id!` in `metaDAOproject/programs` `programs/futarchy/src/lib.rs` @ tag `v0.6.0` + `Anchor.toml [programs.localnet].futarchy`; Cargo `version = "0.6.0"`. On-chain: slot 423005106, 1243500 bytes, upgrade auth `6awyHMsh…` (same MetaDAO authority as v0.4). |
| conditional_vault (v0.6 line) | `VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg` | UNCHANGED from v0.4 — `declare_id!` @ `v0.6.0` identical to v0.4. On-chain: slot 399213625, 424952 bytes. Split/merge/redeem + init/resolve_question discriminators are byte-for-byte the v0.4 ones (re-verified). |
| Meteora DAMM v2 (cp-amm) | `cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG` | `declare_id!` in `MeteoraAg/damm-v2` `programs/cp-amm/src/lib.rs` @ main; cross-confirmed as mainnet deployment in `MeteoraAg/damm-v2-sdk`. MetaDAO's `programs/damm_v2_cpi` shim (v0.6 tree) `declare_id!`s the same address. On-chain: slot 428936648, 2174352 bytes, upgrade auth `JADaUV8k…` (Meteora's). |
| Squads v4 (DAO execution-authority host) | `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf` | `declare_id!` in `Squads-Protocol/v4` @ rev `6d5235da621a2e9b7379ea358e48760e981053be` (the rev `futarchy/Cargo.toml` pins). Seeds from `state/seeds.rs`. |

**Two recon findings that REVISE the design:**
1. **DAO execution authority = a Squads v4 multisig vault, NOT a futarchy PDA.** `initialize_dao` CPIs into Squads to create a multisig whose `create_key` is the `Dao` PDA; a passed proposal carries a `squads_proposal` and executes through the Squads **vault** PDA. So `Protocol.dao_authority` (signer of `set_config`/`resolve_deadend`, F1/F3/F4/F6) is the Squads vault PDA. Seeds (under `SQDS4…`): multisig `[b"multisig", b"multisig", dao]`; vault `[b"multisig", multisig, b"vault", [0u8]]`. DAO PDA (under futarchy) `[b"dao", dao_creator, nonce_le8]`. Proposal PDA `[b"proposal", squads_proposal]`.
2. **Meteora cp-amm has NO TWAP oracle.** Its zero-copy `Pool` exposes only an INSTANTANEOUS `sqrt_price: u128` (Q64.64) + cumulative *fee* accumulators — no cumulative price observation. The manipulation-resistant KASS/USDC TWAP the design's `kass_price` (F5) needs is the futarchy program's **embedded** `FutarchyAmm` spot-pool `TwapOracle` (`Dao.amm`), not Meteora. **F5 should read the futarchy spot TWAP**, not a Meteora pool. The `kass_usdc_pool`/`kass_price` design language should be re-read as "the futarchy DAO's embedded spot market".

**Layouts documented (from v0.6 / Meteora / Squads source, in `src/cpi/metadao_v06.rs`):**
- **Futarchy spot TWAP (the real F5 source)** — the spot `Pool` is the first payload element of both `PoolState` variants, so its offsets are FIXED regardless of variant: in a `Dao` account, byte 8 = PoolState tag (0=Spot,1=Futarchy), byte 9 = spot `Pool`/`TwapOracle` start: aggregator u128@9, last_updated_timestamp i64@25, created_at_timestamp i64@33, last_price u128@41, last_observation u128@57, max_observation_change_per_update u128@73, initial_observation u128@89, start_delay_seconds u32@105. `get_twap = aggregator / (last_updated_ts − (created_at_ts + start_delay_seconds))`, price scaled ×1e12. Implemented + validated as `futarchy_spot_twap`.
- **`Dao` account** — field order documented; CAUTION `amm.state` is a Borsh enum (Spot=1+132, Futarchy=1+3×132 bytes) so ALL fields after it (`squads_multisig_vault`, mints, params…) are at VARIABLE offsets. F1 must store the vault key in `Protocol` at bootstrap rather than re-derive from `Dao` bytes.
- **`Proposal` account** — `number` u32@8, `proposer` Pubkey@12, `timestamp_enqueued` i64@44, `state` enum tag@52 (0=Draft{+8},1=Pending,2=Passed,3=Failed); fields after `state` are variable-offset.
- **Meteora cp-amm `Pool`** — full field ORDER documented; the load-bearing field is `sqrt_price: u128`. Exact byte offset NOT hand-pinned (nested zero-copy `PoolFeesStruct` C-padding is error-prone by hand); deferred to F5 to pin against a LIVE pool dump / published IDL IF F5 ends up reading Meteora at all (it likely won't — finding #2).

**Discriminators (computed `sha256("global:<name>")[..8]` / `account:<Type>`, in the module):** futarchy initialize_dao/initialize_proposal/launch_proposal/finalize_proposal/update_dao/spot_swap/conditional_swap; account Dao/Proposal; Meteora initialize_pool/swap/add_liquidity + account Pool. Vault discs reused from `cpi::metadao` (unchanged).

**Files (all additive; v0.4 integration untouched):**
- `scripts/fetch-metadao-v06.sh` — documents IDs/versions/sources/slots + sha256 pins; dumps `metadao_futarchy_v06.so` (1243500 B), `metadao_conditional_vault_v06.so` (424952 B), `meteora_damm_v2.so` (2174352 B) into `programs/kassandra/tests/fixtures/`.
- `programs/kassandra/src/cpi/metadao_v06.rs` (+ `pub mod metadao_v06;`) — IDs, discriminators, PDA seed builders + derivation, the `futarchy_spot_twap` reader, no-alloc arg encoders (`initialize_dao_data_no_limit` for the `None` spending-limit case; complex/variable-length params STUBBED + documented), invoke wrappers.
- `programs/kassandra/tests/metadao_v06_cpi.rs` — 5 tests, all green.

**Validated vs deferred:**
- VALIDATED against real binaries: all 3 fixtures load + executable; a FULL conditional_vault split (initialize_question → initialize_conditional_vault → split_tokens) against the v0.6-dumped vault binary; the COMPUTED futarchy `initialize_dao` discriminator dispatches into the real futarchy binary's `InitializeDao` handler (asserted via the `Instruction: InitializeDao` program log) while a bogus discriminator does not; the futarchy spot-TWAP offset map + `get_twap` math against a hand-built `Dao` blob.
- DEFERRED to F5/F6: a full `initialize_dao` success (needs Squads v4 program + mints loaded), driving a proposal to pass/execute, and reading a live Meteora cp-amm `sqrt_price` (cp-amm has no TWAP — see finding #2).

Build: `just build` (SBF) clean. Tests: `cargo test -p kassandra-program` all pass (incl. the 5 new). `cargo clippy --all-targets` clean; `cargo fmt` applied.
