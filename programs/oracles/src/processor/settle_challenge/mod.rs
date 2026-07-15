//! `settle_challenge`: settle ONE challenged claim's decision market after its
//! TWAP window, applying the slash trigger and resolving the MetaDAO question.
//!
//! Settlement is **incremental** across markets (design §6): each call settles
//! exactly one [`Market`] and updates oracle state immediately. The phase STAYS
//! [`Phase::Challenge`]; the final plurality recompute + phase transition is
//! Task 12's `finalize_oracle`, which runs after every market has settled.
//!
//! # Hard AMM verification (the deferred Task-10 requirement)
//! `open_challenge` only checked `owner == AMM_ID` on the two AMMs (the v0.4 AMM
//! layout was not yet pinned). Here we read the real `Amm` layout (see the
//! offset consts in [`crate::cpi::metadao`]) and BIND each AMM to THIS market's
//! conditional mint pair:
//! * `pass_amm.base_mint  == conditional_token_mint_pda(kass_vault, 0)` (pass-KASS)
//! * `pass_amm.quote_mint == conditional_token_mint_pda(usdc_vault, 0)` (pass-USDC)
//! * `fail_amm.base_mint  == conditional_token_mint_pda(kass_vault, 1)` (fail-KASS)
//! * `fail_amm.quote_mint == conditional_token_mint_pda(usdc_vault, 1)` (fail-USDC)
//!
//! and require `pass_amm != fail_amm`. A challenger therefore cannot point
//! settlement at an AMM they control: the TWAP read is on the canonical pass/fail
//! pools of this exact market.
//!
//! # TWAP read (no crank needed at settle)
//! The v0.4 AMM stores a slot-weighted price aggregator. `get_twap()` in the AMM
//! source is `aggregator / (last_updated_slot - created_at_slot)` — already the
//! finalized time-weighted average; we read those stored fields directly. We do
//! NOT crank here: cranking only matters to *fold in* the most recent price
//! before reading, but (a) a crank only records once per `ONE_MINUTE_IN_SLOTS`
//! and (b) the design's manipulation resistance comes precisely from NOT letting
//! a last-moment observation dominate the window average. Trading parties (or a
//! permissionless cranker) keep the observation fresh during the window; settle
//! consumes the stored average. If a market never traded (`aggregator == 0` or
//! zero slots elapsed) its TWAP reads as `0` — "challenge market with no
//! counter-trading → claim survives" (design §7).
//!
//! # Slash trigger (design §6, invariant §9.8)
//! Disqualify iff `fail_twap > pass_twap + threshold`, with the protocol-global
//! relative margin from [`crate::config`]: `fail_twap * DEN > pass_twap * (DEN +
//! NUM)`, computed in `u128` (the TWAPs are already `u128`).
//! * **Disqualified (fraud):** `proposer.disqualified = slashed = 1`; the
//!   proposer's full bond (split into conditional KASS at `open_challenge`) is
//!   forfeit to `oracle.bond_pool`; `surviving_count -= 1`. The question resolves
//!   FAIL-side (`[0, 1]`) so the fail-conditional tokens become redeemable.
//! * **Survives (honest):** no slash; the question resolves PASS-side (`[1, 0]`).
//!
//! `slashed_amount` is kept consistent with Task 7's per-proposer accounting
//! (a proposer's `bond_pool` contribution always equals its `slashed_amount`).
//! With the C2 KASS-fee carve-out (below), that contribution is `bond −
//! kass_fee`: we add only `(bond − kass_fee) − already_slashed` so a previously
//! flip-slashed proposer is topped up to exactly `bond − kass_fee`, never
//! double-counted, and the identity `slashed_amount == bond_pool contribution`
//! still holds.
//!
//! # Physical settlement + directional fees (Task C2 — implemented here)
//! After `resolve_question`, settle PHYSICALLY redeems the bond's idle pass/fail
//! conditional KASS (`market.oracle_pass_kass` + `oracle_fail_kass`) back into
//! `oracle.stake_vault` via a program-signed `redeem_tokens` CPI (winning side
//! redeems 1:1, losing side → 0, so the FULL `bond` KASS lands in `stake_vault`;
//! the bond was split into BOTH legs at `open_challenge` and never traded, so the
//! redeem is clean — recon §3/§4). Then it routes the directional fees:
//! * **Survives (pass-win, challenge FAILED):** the bond is the proposer's (no
//!   slash). `usdc_fee = challenger_usdc × challenge_fail_usdc_fee_num/den` →
//!   PROPOSER's USDC account; `challenger_usdc − usdc_fee` → CHALLENGER's USDC
//!   account. (Escrow fully accounted: fee + return == escrow.)
//! * **Disqualified (fail-win, challenge SUCCEEDED):** `kass_fee = bond ×
//!   challenge_success_kass_fee_num/den` → CHALLENGER's KASS account (from
//!   `stake_vault`); `bond − kass_fee` is the proposer's `bond_pool` contribution
//!   (== `slashed_amount`). The FULL `challenger_usdc` escrow → CHALLENGER's USDC
//!   account. (No proposer USDC fee on a successful challenge.)
//!
//! All token moves are program-signed by the oracle PDA (the SPL authority of
//! `stake_vault`, the escrow vault, and the conditional-KASS destinations).
//!
//! # Conservation
//! * KASS: redeem lands `bond` in `stake_vault`; on disqualify `kass_fee` then
//!   leaves to the challenger, so `stake_vault + kass_vault_underlying + kass_fee
//!   == total_oracle_stake`; on survive nothing leaves, so `stake_vault +
//!   kass_vault_underlying == total_oracle_stake`.
//! * USDC: `challenger_usdc == challenger_return + proposer_fee` (survive) or
//!   `== challenger_return + 0` (disqualify), exactly.
//!
//! # Accounts
//! 0.  oracle              — writable; owned by this program; the question's
//!     resolver + SPL authority of stake_vault/escrow/conditional dests
//! 1.  market              — writable; the [`Market`] PDA for this claim
//! 2.  ai_claim            — read-only; `== market.ai_claim`
//! 3.  proposer            — writable; `== market.proposer`
//! 4.  question            — writable; `== market.question` (resolved here)
//! 5.  pass_amm            — read-only; `== market.pass_amm`, owned by `AMM_ID`
//! 6.  fail_amm            — read-only; `== market.fail_amm`, owned by `AMM_ID`
//! 7.  conditional_vault program
//! 8.  cv_event_authority  — read-only; conditional_vault `#[event_cpi]` authority
//! 9.  token program
//! 10. stake_vault         — writable; `== oracle.stake_vault` (redeem dest + KASS-fee source)
//! 11. kass_vault          — writable; `== market.kass_vault` (redeem vault)
//! 12. kass_vault_underlying — writable; `== kass_vault.underlying_token_account`
//! 13. pass_kass_mint      — writable; conditional-KASS mint idx 0 of kass_vault
//! 14. fail_kass_mint      — writable; conditional-KASS mint idx 1 of kass_vault
//! 15. oracle_pass_kass    — writable; `== market.oracle_pass_kass` (pass-KASS holder)
//! 16. oracle_fail_kass    — writable; `== market.oracle_fail_kass` (fail-KASS holder)
//! 17. challenger_usdc_vault — writable; `== market.challenger_usdc_vault` (USDC escrow)
//! 18. proposer_usdc       — writable; proposer's USDC account (mint==usdc, owner==proposer.authority)
//! 19. challenger_usdc_dest — writable; challenger's USDC account (mint==usdc, owner==market.challenger)
//! 20. challenger_kass     — writable; challenger's KASS account (mint==kass, owner==market.challenger)
//!
//! # Instruction payload (after the 1-byte discriminant)
//! `oracle_nonce: u64 LE` (exactly 8 bytes) — the oracle PDA signer seed nonce,
//! verified by re-derivation (same scheme as `open_challenge`).

mod entry;
mod twap;

pub use entry::process;
