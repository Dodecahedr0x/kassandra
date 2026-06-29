# Kassandra Challenge-Market Rework — Design + Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete the challenge decision-market economics: a **conditional market on the proposer's stake** (pass-KASS/fail-KASS priced in conditional USDC — traders price the bond's value conditional on the claim surviving vs being disqualified), with the **challenger escrowing USDC** (sized via `kass_price`), **physical settlement** (the deferred `redeem_tokens`), and **directional fees**. Keeps the v0.4 AMM + TWAP slash trigger (already built). Second step of the roadmap: KASS futarchy ✅ → **challenge-market rework** → staker settlement.

**Architecture:** Extends the existing Pinocchio program. The challenge market reuses MetaDAO **v0.4** conditional vault + AMM (the v0.4 AMM has the built-in TWAP the slash reads; Meteora has none). Bond stays clean-slashable; the conditional-stake model is preserved.

**Tech Stack:** Rust, `pinocchio` 0.8, `bytemuck`, `litesvm`, `solana-sdk` (test-only), `spl-token`, MetaDAO v0.4 conditional vault + AMM, `kass_price` (futarchy spot TWAP, from the merged futarchy milestone).

**Source of truth:** the recon findings `docs/plans/2026-06-29-challenge-rework-recon.md`; the dispute-core + happy-path + futarchy deltas (live state); `docs/plans/2026-06-29-kassandra-settlement-economics.md` (the broader settlement note). FOLLOW THE LIVE STATE.

---

## Validated design (brainstormed + recon-grounded)

### The conditional-stake market (NOT plain KASS/USDC)
- `open_challenge` splits the proposer's **bond** into **pass-KASS / fail-KASS** conditional tokens (as it does today). The pass/fail AMMs price pass-KASS in pass-USDC and fail-KASS in fail-USDC — i.e. **traders price a unit of the proposer's stake conditional on the claim surviving (pass) vs being disqualified (fail).** pass/fail-KASS are fungible across participants, so the TWAP reflects the conditional value of the stake regardless of whose tokens trade.
- **The bond's conditional tokens stay IDLE (never LP'd)** → no impermanent loss on the bond (recon finding: LP'ing the bond makes it unrecoverable; holding idle + redeeming is the clean "escrow/idealized" model — and it's what's built). 
- **Market liquidity is the CHALLENGER's** (+ traders'): their conditional KASS + conditional USDC seed the pools (out-of-band, as the current tests do) — their IL, never the bond's.
- **Slash trigger (unchanged):** TWAP of fail-stake-price vs pass-stake-price; disqualify iff `fail_twap * DEN > pass_twap * (DEN + NUM)` (the `oracle.market_threshold_*` snapshot). `pass_twap == 0` → survive (no counter-trading).

### Challenger USDC stake
- `open_challenge` **escrows the challenger's USDC** into a market-owned USDC vault, amount sized via `kass_price` (≈ the bond's KASS value, so both sides have comparable skin-in-the-game). This escrow is the source of the USDC directional fee and is returned (minus fee) at settle.

### Physical settlement + directional fees (settle_challenge)
Implements the previously-deferred `redeem_tokens` + adds fees:
- **Redeem the bond's idle conditional tokens** 1:1 on the resolved (winning) side → underlying KASS into `stake_vault`.
- **Survives (challenge failed):** bond stays the proposer's (no slash); **USDC fee** = `challenger_usdc × fail_usdc_fee_num/den` → proposer; remaining challenger USDC escrow → returned to challenger.
- **Disqualified (challenge succeeded):** bond → `bond_pool` **minus a KASS fee** = `bond × success_kass_fee_num/den` → challenger; challenger's USDC escrow returned in full. (`slashed_amount` accounting stays consistent: the proposer's bond_pool contribution = bond − kass_fee; document the fee as a carve-out, and keep the per-proposer identity.)
- Directional-fee rates are **governable config** (new snapshot fields).

### Invariants
- Bond is never AMM liquidity → clean slashing + KASS conservation preserved (extended to count the market USDC escrow + the redeemed conditional KASS).
- Challenger USDC escrow is conserved: returned to challenger + fee to proposer == escrowed amount.

---

## Conventions (unchanged)
TDD; `just build` before `cargo test`; clippy + fmt clean; commit trailer `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`, git author `Kassandra <hexadecifish@gmail.com>`; append-only Ix/error discriminants; re-pin `tests/state_layout.rs` on layout change. rust-analyzer false positives — rely on real cargo runs.

## Live-state entry points
- `Ix` up to `KassPrice=16`; `KassandraError` up to `InvalidConfig=26`. `Protocol` LEN 336, `Oracle` LEN 328 (governable params snapshot). `Market` LEN 384 (records oracle/ai_claim/proposer/challenger/question/kass_vault/usdc_vault/pass_amm/fail_amm/oracle_pass_kass/oracle_fail_kass/twap_end/challenger_usdc/settled/bump).
- `open_challenge` (Ix=4): splits bond→idle conditional KASS, records Market (incl. `challenger_usdc` amount, currently NOT escrowed), `challenged=1`. `settle_challenge` (Ix=5): TWAP slash + `resolve_question`; **physical redeem + fees DEFERRED** (this milestone).
- `kass_price(protocol, kass_dao_ai) -> u128` (futarchy spot TWAP), anchored to `Protocol.kass_dao`. `assert_dao_authority`, `set_config` (governable params), `load_protocol/oracle/...`, `create_pda`. v0.4 CPI in `src/cpi/metadao.rs` (split/merge/redeem discriminators incl. `redeem_tokens` `f6 62 86 29 98 21 78 45`, add/remove-liquidity shapes documented in the recon doc).

---

## Tasks (C0 recon DONE)

### C1 — Challenger USDC escrow + fee config
- **Add governable fee fields** to `Protocol` + `Oracle` (snapshot; re-pin both layouts): `challenge_fail_usdc_fee_num/den` (USDC fee on a failed challenge), `challenge_success_kass_fee_num/den` (KASS fee on a successful challenge). Default to sensible config consts (e.g. 1/100 = 1%). `init_protocol` defaults them; `create_oracle` snapshots; `set_config` updates them with bounds (den>0, num≤den) — extend its payload + bounds. Update the F3 set_config payload length/tests.
- **`Market`** gains a `challenger_usdc_vault: Pubkey` (the market-owned USDC escrow token account) — re-pin Market layout. (Or reuse an existing field if cleaner; document.)
- **`open_challenge`:** add accounts for `protocol` + `kass_dao` (to call `kass_price`) + the challenger's USDC source token account + the market USDC escrow vault (created here, owned by the market/oracle PDA). Compute the required escrow = `bond_kass × kass_price` converted across KASS 9dp / USDC 6dp / the TWAP scale (DOCUMENT the exact conversion + scale; use u128, overflow-safe). Transfer that USDC challenger→escrow (challenger signs). Reject if the challenger's `challenger_usdc` payload disagrees with the computed size beyond a tolerance, OR just compute it and ignore the payload field (document). Keep the existing bond split + market binding + `challenged=1`.
- Tests: open_challenge escrows the right USDC amount (sized by a known kass_price); under/over-funded challenger → fails; fee config snapshotted onto the oracle; set_config updates fee rates (bounds enforced).

### C2 — settle_challenge: physical redeem + directional fees
- **Implement `redeem_tokens`** (the deferred CPI): after `resolve_question`, redeem the bond's idle pass/fail conditional KASS (`oracle_pass_kass`/`oracle_fail_kass`) → underlying KASS into `stake_vault`, program-signed by the oracle PDA. Winning side 1:1, losing side → 0 (recon-confirmed). Net: the full bond KASS lands back in `stake_vault`.
- **Directional fees + routing:**
  - **Survives** (pass-win): bond stays the proposer's (already counted as surviving). Take `fail_usdc_fee = challenger_usdc × fail_usdc_fee_num/den` from the escrow → transfer to the proposer (or the proposer's claimable balance — match how staker settlement will claim; for now, transfer to a proposer-controlled account OR credit a counter — DOCUMENT, keeping it consistent with the deferred staker-settlement milestone). Return `challenger_usdc − fee` → challenger.
  - **Disqualified** (fail-win): bond → `bond_pool` (already counted) **minus** `success_kass_fee = bond × success_kass_fee_num/den` → challenger (transfer KASS from `stake_vault`, program-signed); adjust `bond_pool`/`slashed_amount` so the proposer's contribution == bond − kass_fee (keep the per-proposer identity; the fee is a carve-out to the challenger, documented). Return the full `challenger_usdc` escrow → challenger.
- Update conservation: the market USDC escrow is fully accounted (challenger return + proposer fee == escrow); the redeemed bond KASS is in `stake_vault`.
- Tests: fraud path (disqualified) → bond−kass_fee to bond_pool, kass_fee to challenger, full USDC returned, conditional KASS redeemed; honest path (survives) → bond intact, usdc_fee to proposer, USDC remainder returned; conservation asserted (KASS + USDC).

### C3 — End-to-end + conservation/invariant update
- E2E challenge test driving the REAL v0.4 AMM: open_challenge (with USDC escrow) → challenger seeds liquidity + swaps to drive the TWAP → crank → warp → settle_challenge → assert the full physical settlement + fees for BOTH outcomes.
- Extend the invariant fuzz / conservation assertions to cover the challenge path (KASS: stake_vault + bond_pool reconciles incl. the kass_fee carve-out; USDC: escrow == challenger_return + proposer_fee).
- Remove or fold in the throwaway `tests/recon_lp_resolution.rs` recon test (keep if it documents the IL finding usefully; else drop).

---

## Out of scope (later)
- Staker settlement (per-staker claim/return/reward, emissions) — the broader settlement note. The challenger KASS-fee / proposer USDC-fee land where that milestone's claim model expects (document the hand-off precisely).
- Migrating challenge markets to v0.6/Meteora (kept on v0.4 for the TWAP).

## Execution note
After each task: `just build` → `cargo test` → clippy/fmt, green, commit. Re-pin layouts on change. The kass_price→USDC sizing conversion (decimals/scale) and the redeem_tokens CPI are the two trickiest spots — validate against the real binary. Append a C1/C2/C3 delta log here.

---

## C1 delta log — challenger USDC escrow + governable challenge fees (DONE)

### kass_price units/scale + the USDC conversion (the load-bearing bit)
- `kass_price(&Protocol, kass_dao_ai) -> u128` returns the futarchy spot TWAP =
  `aggregator / seconds_elapsed`, which is a price in **raw quote units per raw
  base unit, scaled by `KASS_PRICE_SCALE = 1e12`** (`futarchy_spot_twap`'s
  `PRICE_SCALE`). For the KASS DAO base = KASS (9dp), quote = USDC (6dp), so the
  value is **raw-USDC per raw-KASS × 1e12** — the cross-decimal (9dp↔6dp)
  adjustment is ALREADY folded into the raw price, so NO extra `10^Δdec` factor
  is needed.
- **Conversion (overflow-safe, u128 intermediate):**
  `required_usdc (USDC base units) = bond_kass (KASS base units) × twap / KASS_PRICE_SCALE`,
  then checked back into `u64`. `bond_kass == proposer.bond`.
- **Worked example (the test price):** KASS at $0.50 → `twap = 500_000_000`; a
  1 KASS bond (`1e9` base units) escrows `1e9 × 5e8 / 1e12 = 500_000` USDC base
  units = $0.50. Dimensionally `[KASS_raw] × [USDC_raw/KASS_raw] = [USDC_raw]`.
- New const `config::KASS_PRICE_SCALE = 1_000_000_000_000`.

### New config consts (`config.rs`)
- `KASS_PRICE_SCALE = 1e12`.
- `CHALLENGE_FAIL_USDC_FEE_NUM/DEN = 1/100` (1% USDC fee on a failed challenge → proposer; routed at C2).
- `CHALLENGE_SUCCESS_KASS_FEE_NUM/DEN = 1/100` (1% KASS fee on a successful challenge → challenger; routed at C2).

### Layout re-pins (each adds fields; `tests/state_layout.rs` updated)
- **Protocol** `336 → 368`: appended 4 × u64 after `reward_fact_weight@328`:
  `challenge_fail_usdc_fee_num@336`, `_den@344`, `challenge_success_kass_fee_num@352`, `_den@360`.
- **Oracle** `328 → 360`: appended the same 4 × u64 after `reward_fact_weight@320`:
  `challenge_fail_usdc_fee_num@328`, `_den@336`, `challenge_success_kass_fee_num@344`, `_den@352`.
  (`init_protocol` defaults the Protocol copies; `create_oracle` snapshots them onto the Oracle.)
- **Market** `384 → 416`: inserted `challenger_usdc_vault: Pubkey @360` (after
  `oracle_fail_kass@328`); shifting `twap_end@392`, `challenger_usdc@400`,
  `settled@408`, `bump@409`, `_pad[6]@410`.

### Escrow vault
- PDA seeds **`[b"challenge_usdc", market]`** (program = `crate::ID`); SPL token
  account on `oracle.usdc_mint`, **token authority = the oracle PDA** (mirrors
  `oracle.stake_vault`, so C2 settle signs returns/fees with the oracle seeds).
  Created in `open_challenge` via `create_pda` + `InitializeAccount3` (rent paid
  by the challenger), then funded by a challenger-signed SPL `Transfer` of
  `required_usdc`. Under-funded source → the `Transfer` fails → whole ix reverts.
- `Market.challenger_usdc` is now the ON-CHAIN-computed amount (not a payload
  value); `Market.challenger_usdc_vault` records the escrow account.

### `open_challenge` account order (Ix=4) — appended 5 accounts; payload now nonce-only (8 bytes)
`0 oracle(w) · 1 ai_claim(w) · 2 proposer(w) · 3 market(w) · 4 challenger(signer,w) ·
5 question · 6 kass_vault(w) · 7 usdc_vault · 8 pass_amm · 9 fail_amm · 10 stake_vault(w) ·
11 kass_vault_underlying(w) · 12 pass_mint(w) · 13 fail_mint(w) · 14 oracle_pass_kass(w) ·
15 oracle_fail_kass(w) · 16 cv_program · 17 token_program · 18 system_program ·
19 cv_event_authority · 20 protocol · 21 kass_dao · 22 usdc_mint · 23 challenger_usdc_src(w) ·
24 challenger_usdc_vault(w, uninit, created here)`. The escrow is sized + created
AFTER all MetaDAO bindings are verified (no funds move before validation).
Payload dropped the legacy `challenger_usdc` field (compute-on-chain is cleaner).

### set_config payload growth + new bounds
- Payload `18 → 22` u64 fields (`144 → 176` bytes); 4 appended:
  `challenge_fail_usdc_fee_num/den`, `challenge_success_kass_fee_num/den`.
- New bounds (→ `InvalidConfig`): `challenge_fail_usdc_fee_den > 0`,
  `challenge_success_kass_fee_den > 0`, `challenge_fail_usdc_fee_num ≤ den`,
  `challenge_success_kass_fee_num ≤ den`.
- Harness `ConfigParams` grew the 4 fields + `to_payload` is now `[u8; 176]`.

### Tests
- `open_challenge.rs`: happy path now asserts escrow == `bond × kass_price` USDC
  in the vault + Market record + challenger debit + vault mint/authority; new
  `open_challenge_insufficient_usdc_fails` (under-funded source reverts, no
  Market). All existing open_challenge + settle_challenge tests updated for the
  new accounts (harness `bless_kass_price` blesses a deterministic futarchy Dao
  blob; `fund_usdc` funds the challenger).
- `set_config.rs`: default-fee snapshot, fee update + new-oracle snapshot, and
  den==0 / num>den rejection.
- C2 (settle-side fee routing / redeem) intentionally NOT implemented here.

---

## C2 delta log — settle_challenge physical redeem + directional fees (DONE)

### redeem_tokens CPI (validated against the real v0.4 binary)
- `redeem_tokens` (disc `f6 62 86 29 98 21 78 45`, NO args) uses the SAME
  `InteractWithVault` account struct as `split_tokens` — VERIFIED against the
  deployed v0.4 `conditional_vault` source (`instructions/common.rs` +
  `redeem_tokens.rs` fetched at tag `v0.4`). It is gated by
  `question.is_resolved()`, burns the holder's FULL balance of EVERY outcome's
  conditional token, and transfers `Σ_i balance_i × payout_numerators[i] /
  payout_denominator` underlying out. For binary pass-wins `[1,0]`: pass redeems
  1:1, fail → 0; fail-wins `[0,1]` symmetric. The bond was split into BOTH legs
  at open and never traded, so the redeem is CLEAN → the FULL `bond` KASS lands in
  `stake_vault`. New encoder `metadao::redeem_tokens_data() -> [u8;8]`.
- Account order (program-signed by the oracle PDA): `0 question(ro) · 1
  kass_vault(w) · 2 kass_vault_underlying(w) · 3 authority=oracle PDA(signer) · 4
  stake_vault(w, user_underlying) · 5 token_program · 6 cv_event_auth · 7
  cv_program · 8 pass_kass_mint(w) · 9 fail_kass_mint(w) · 10 oracle_pass_kass(w) ·
  11 oracle_fail_kass(w)`. `user_underlying` (stake_vault) + the conditional
  holders are all owned by the oracle PDA (the InteractWithVault
  `token::authority = authority` constraint), so the redeemed KASS lands in
  stake_vault. DRIVEN end-to-end against the real binary for BOTH outcomes (the
  one piece the recon flagged as not-yet-driven is now driven).

### Fee routing (both outcomes)
- **Survives (pass-win, challenge FAILED):** `usdc_fee = challenger_usdc ×
  challenge_fail_usdc_fee_num/den` → PROPOSER's USDC account; `challenger_usdc −
  usdc_fee` → CHALLENGER's USDC account (both from the escrow vault, oracle-PDA
  signed). Bond stays the proposer's (redeemed into stake_vault). USDC
  conservation: `usdc_fee + return == challenger_usdc`, exactly.
- **Disqualified (fail-win, challenge SUCCEEDED):** `kass_fee = bond ×
  challenge_success_kass_fee_num/den` KASS from stake_vault → CHALLENGER's KASS
  account; the FULL `challenger_usdc` escrow → CHALLENGER's USDC account (no
  proposer USDC fee). Fee rates read from the per-ORACLE snapshot (governable).

### slashed_amount / bond_pool adjustment (the carve-out)
- The disqualify block now slashes `net_slash = bond − kass_fee` (not the full
  bond): `delta = net_slash − already_slashed`, `slashed_amount = net_slash`,
  `bond_pool += delta`. The per-proposer identity `slashed_amount == bond_pool
  contribution` HOLDS with the carve-out; `kass_fee` physically leaves stake_vault
  to the challenger. KASS conservation becomes `stake_vault + kass_vault_underlying
  + kass_fee == total_oracle_stake` on disqualify (the original
  `stake_vault + underlying == total` still holds on survive).

### settle account order (Ix=5) — appended 12 accounts; payload still nonce-only (8B)
`0 oracle(w) · 1 market(w) · 2 ai_claim · 3 proposer(w) · 4 question(w) · 5
pass_amm · 6 fail_amm · 7 cv_program · 8 cv_event_authority · 9 token_program · 10
stake_vault(w) · 11 kass_vault(w) · 12 kass_vault_underlying(w) · 13
pass_kass_mint(w) · 14 fail_kass_mint(w) · 15 oracle_pass_kass(w) · 16
oracle_fail_kass(w) · 17 challenger_usdc_vault(w, escrow) · 18 proposer_usdc(w) ·
19 challenger_usdc_dest(w) · 20 challenger_kass(w)`. The three payout token
accounts are bound by mint + owner (`proposer_usdc ↔ proposer.authority`,
`challenger_usdc/kass ↔ market.challenger`); stake_vault/kass_vault/escrow/
conditional holders are pinned to the recorded `Oracle`/`Market`, so a settle
cranker cannot redirect funds. New `fee_amount(value, num, den)` helper (u128,
checked, den==0 → InvalidConfig).

### C1-carryforward fixes
- `open_challenge`: reject `required_usdc == 0` → `ZeroStake` (sub-micro/zero
  bond escrow truncates to nothing; no fee source at settle). New test
  `open_challenge_zero_escrow_fails` (bond=1 → escrow 0 → ZeroStake, no Market).
- `open_challenge`: `KNOWN LIMITATION` comment on the escrow `create_pda`
  (pre-funding griefing, matching propose/submit_fact convention).
- Documented the POOL-ORIENTATION assumption (kass_price = USDC-per-KASS because
  the blessed `kass_dao` spot pool is KASS-base/USDC-quote) where the escrow is
  sized, plus the DOWNWARD-truncation note on the escrow value.

### Tests (settle_challenge.rs)
- `settle_fraud_*` + `settle_honest_*` extended with the full physical-redeem +
  fee assertions (redeem drains the conditional KASS vault + burns both holders;
  KASS routing incl. the kass_fee carve-out; USDC routing + exact conservation
  for both outcomes). New `settle_fee_rates_are_oracle_snapshotted` (retune the
  oracle snapshot to 5% KASS / 2% USDC → settle's fee tracks it). Existing
  before-window / double-settle / AMM-binding-attack / aliased-AMM / last-block-
  swap / uncranked-pass tests intact. All driven against the REAL v0.4 AMM +
  conditional_vault binaries in LiteSVM.

---

## C3 delta log — e2e challenge lifecycle (real AMM, both outcomes) + conservation fuzz (DONE)

All TEST work; NO production change (no genuine bug surfaced — see below).

### `tests/challenge_e2e.rs` — full-lifecycle e2e, BOTH outcomes
- **`e2e_honest_full_lifecycle_survives`** + **`e2e_fraud_full_lifecycle_swap_driven_disqualifies`**.
- **Real vs seeded split (honest):**
  - **Dispute core (front door) is REAL** — `front_door_to_challenge` drives
    `create_oracle → propose×2 (conflict) → finalize_proposals → submit_fact →
    advance_phase → vote_fact → finalize_facts → submit_ai_claim×2 →
    finalize_ai_claims` to land `Phase::Challenge` with a real `AiClaim` for a
    surviving, UN-slashed proposer (option-0 proposer claims option 0 → no flip).
    NO `set_phase` shortcut in the e2e tests; only `warp`/`warp_slots` move time.
  - **MetaDAO market + AMMs are REAL** — composed via real CPIs; `open_challenge`
    verifies/records them, escrows the challenger USDC (sized by `kass_price`),
    and program-signs the bond split.
  - **`open_challenge → settle_challenge` is REAL** — `resolve_question` +
    `redeem_tokens` + directional-fee transfers, all program-signed real CPIs
    against the deployed v0.4 binaries.
  - **TWAP is REAL and SWAP-DRIVEN on the fraud path** — the fail pool's price is
    moved by a genuine `swap` (BUY) and accumulated into the slot-weighted TWAP
    across TWO `crank_that_twap` calls 300 slots apart (TWAP ≈ (1e9 + ~3.5e9)/2 ≫
    pass·1.1), so the disqualify decision is driven by real trading, not a seeded
    price. (Single-crank cannot move the TWAP — the aggregator weights the
    pre-swap observation over the whole window; the 2-crank accumulation with
    fixed 300-slot warps is the deterministic way to make a swap load-bearing,
    sidestepping the brittleness the `settle_challenge.rs` NOTE flags.) Honest
    path leaves both pools neutral (pass == fail → survive).
- **Conservation vs an INDEPENDENT reference** — `ConservationModel` predicts
  every post-settle token delta from `bond` + escrow + the fee config ALONE
  (never reads the program's accounting). Both equations asserted:
  - KASS: `stake_vault + kass_vault_underlying + challenger_kass == total_oracle_stake`
    (the kass_fee carve-out left to the challenger on disqualify; on survive
    `challenger_kass == 0` → the idle-bond conservation).
  - USDC: `proposer_usdc + challenger_usdc_dest == escrow`, exactly.
  Plus redeem-drained holders, `slashed_amount`/`bond_pool` identity, and the
  question resolution (`[1,0]` survive / `[0,1]` disqualify).

### Conservation FUZZ — `challenge_conservation_fuzz` (24 cases)
- Sweeps `bond` × `pass_twap`/`fail_twap` (incl. `pass_twap==0` → always survive)
  × fuzzed directional fee rates (num ≤ den), driving the REAL `open_challenge`
  (split + escrow) + `settle_challenge` (redeem + directional fees) and asserting
  BOTH conservation equations + the slash outcome against the same independent
  `ConservationModel` + an independent `ref_disqualify` slash rule.
- **TWAP is STUBBED here (documented):** the pass/fail AMMs are FABRICATED
  AMM-program-owned accounts carrying a chosen aggregator, so
  `verify_and_read_twap` reads a known TWAP cheaply. The real-AMM *TWAP
  production* path is covered by the two e2e tests + `settle_challenge.rs`; the
  fuzz targets the fee/conservation ACCOUNTING across both outcomes. Case count
  kept at 24 (each case rebuilds LiteSVM + loads both MetaDAO binaries + composes
  real vaults + real open/settle ≈ 7 txs) to stay fast (~2.5s) and non-flaky.

### Donation edge (C2 review heads-up) — handled, NO production change
- **`donation_into_holder_inflates_stake_vault_not_theft`**: a third party
  `split_tokens` D KASS in this market's KASS vault and SPL-transfers the D
  pass-KASS into the oracle-PDA-owned `oracle_pass_kass` holder before settle.
  `redeem_tokens` burns the FULL holder balance (bond + D) → pulls bond + D
  underlying into `stake_vault`. The test asserts `stake_vault + underlying ==
  total_oracle_stake + D` — i.e. the donation ONLY INFLATES `stake_vault`; the
  donor forfeits their own KASS (their fail-KASS is worthless) and NO protocol
  funds leave to them. It is external griefing that ADDS KASS to the protocol,
  not theft, so production is intentionally NOT guarded (a guard would reject a
  donor gifting the protocol KASS). Judged NOT a vulnerability → no STOP needed.

### Recon test reconciliation
- **KEPT `tests/recon_lp_resolution.rs`** (with a reconciliation note in its
  header): it is the ONLY real-binary `remove_liquidity` coverage and pins the
  impermanent-loss finding that motivated the escrow/idle-bond model now driven
  end-to-end by `challenge_e2e.rs` + `settle_challenge.rs`.

### Build/test
- `just build` + full `cargo test -p kassandra-program` green; `cargo clippy
  --all-targets` clean; `cargo fmt` applied.

---

## Challenge rework: COVERED vs DEFERRED

**Covered (driven against the REAL deployed v0.4 amm + conditional_vault binaries):**
- C1 — challenger USDC escrow sized by `kass_price`, governable challenge fees,
  set_config bounds (`open_challenge.rs`, `set_config.rs`).
- C2 — physical `redeem_tokens` + directional fees (USDC→proposer on survive,
  KASS→challenger on disqualify) + the slashed_amount/bond_pool carve-out
  identity, both outcomes (`settle_challenge.rs`).
- C3 — full e2e lifecycle BOTH outcomes via real front door → real open → real
  AMM (swap-driven fraud TWAP) → real settle; KASS+USDC conservation vs an
  independent reference; conservation fuzz; donation edge documented
  (`challenge_e2e.rs`). Real-binary `remove_liquidity`/IL pin
  (`recon_lp_resolution.rs`).
- TWAP slash trigger: hand-enumerated outcome cases incl. AMM-binding attack,
  aliased pools, last-block-swap manipulation resistance, uncranked-pass survive,
  flip→disqualify no-underflow (`settle_challenge.rs`).

**Deferred (out of scope, documented at the call sites / in the plan):**
- Staker settlement (per-staker claim/return/reward, emissions) — the challenger
  KASS-fee / proposer USDC-fee land in plain token accounts where that milestone's
  claim model will pick them up.
- Multi-crank "spike dilutes over a long window" TWAP test — brittle under LiteSVM
  slot accounting; the realistic last-block attack is covered deterministically.
- Donation griefing guard — judged a non-vulnerability (donor-funded inflation,
  not theft); intentionally no production guard.
- Migrating challenge markets off v0.4 (kept on v0.4 for the built-in TWAP).
