//! `claim_proposer` / `claim_fact` / `claim_fact_vote` (Task S2): the first
//! PHYSICAL payouts of the staker-settlement layer.
//!
//! Each is a PERMISSIONLESS, per-staker PULL: anyone may crank a claim for one
//! account, but the KASS lands in the claimant-owner's token account. A claim
//! (1) requires the oracle to be TERMINAL ([`Phase::Resolved`] or
//! [`Phase::InvalidDeadend`]); (2) loads + type-checks the claimant account and
//! binds it to this oracle; (3) computes the entitlement from the matrix below;
//! (4) transfers exactly that KASS from `stake_vault` (program-signed by the
//! oracle PDA) to the claimant-owner's KASS account; and (5) CLOSES the claimant
//! account, draining its rent lamports to the owner. Idempotent BY CLOSURE — a
//! second claim finds the account gone (zero lamports → reaped) and fails the
//! owner/type guard.
//!
//! # CONSERVATION CONTRACT
//! Every payout is sourced from the real `stake_vault` balance + the per-account
//! `slashed_amount` ledger + the resolution-time stamps (`reward_pool`,
//! `total_correct_proposer_stake`, `total_approved_fact_stake`). NOTHING reads
//! `total_oracle_stake` (an idealized accumulator, NOT physical KASS — a
//! successful challenge / external donation can desync it). Σ entitlements ≤
//! `stake_vault` balance; the floor-division dust stays in the vault.
//!
//! # Per-actor matrix
//! Cohort reward buckets are computed once from the oracle's resolution stamps
//! via [`crate::reward::reward_buckets`]; rewards apply ONLY on `Resolved`.
//!
//! * **claim_proposer** — UNIFORM base `bond − slashed_amount` (any slash already
//!   funded `bond_pool`), plus the cohort reward only when Resolved + surviving +
//!   correct: `entitlement = (bond − slashed_amount) + (resolved &&
//!   !is_disqualified() && claim_option == resolved_option ? proposer_reward(bond,
//!   proposer_bucket, total_correct) : 0)`. So:
//!   - `InvalidDeadend` → `bond − slashed_amount` (= `bond` for an unslashed
//!     proposer; a flip-slashed survivor that tied into a dead-end keeps only the
//!     un-slashed remainder — never the full bond).
//!   - `Resolved` + `is_disqualified()` → `bond − slashed_amount`, no reward.
//!   - `Resolved` + surviving + correct → `(bond − slashed_amount) +
//!     proposer_reward(...)` (= `bond + reward` for an honest survivor;
//!     `bond − flip_slash + reward` for a flip-slashed-but-correct survivor).
//!   - `Resolved` + surviving + wrong → `bond − slashed_amount`, no reward.
//! * **claim_fact** (submitter) — disposition-based on BOTH terminal phases; the
//!   reward applies ONLY on `Resolved`. On `InvalidDeadend` the slashed
//!   `bond_pool` (incl. rejected-fact stakes) was BURNED out of `stake_vault` at
//!   finalize, so a rejected submitter must forfeit (0) to stay solvent.
//!   - `is_agreed()` → `stake + (resolved ? fact_reward(...) : 0)`.
//!   - `is_duplicate()` → `stake` (either phase).
//!   - rejected → `0` (either phase; the stake funded the now-burned `bond_pool`;
//!     still close + reclaim rent to the submitter).
//! * **claim_fact_vote** (the fact is loaded to read its disposition) —
//!   disposition-based on BOTH terminal phases; reward ONLY on `Resolved`.
//!   - `kind == VOTE_DUPLICATE` (any fact) → `stake` (never slashed/rewarded).
//!   - `kind == VOTE_APPROVE` + fact `is_agreed()` → `stake + (resolved ?
//!     fact_reward(...) : 0)`.
//!   - `kind == VOTE_APPROVE` + fact `is_duplicate()` → `stake` (no reward/slash).
//!   - `kind == VOTE_APPROVE` + fact rejected → `stake − floor(stake ·
//!     fact_vote_slash_num / fact_vote_slash_den)` (either phase; the slashed
//!     fraction funded the now-burned `bond_pool`).
//!
//! # Accounts (per claim)
//! `claim_proposer` / `claim_fact`:
//! 0. oracle           — read-only; owned by this program, re-derived from the
//!    payload nonce; the SPL authority of `stake_vault` (signs the payout).
//! 1. claimant         — writable; the `Proposer`/`Fact` account, CLOSED here.
//! 2. dest_kass        — writable; KASS token account, `mint == oracle.kass_mint`
//!    and `owner == claimant.authority` (proposer.authority / fact.proposer).
//! 3. stake_vault      — writable; `== oracle.stake_vault` (the payout source).
//! 4. rent_recipient   — writable; `== claimant.authority` (reclaimed rent).
//! 5. token program.
//!
//! `claim_fact_vote` inserts the fact at index 2 and shifts the rest:
//! 0. oracle, 1. fact_vote(w, closed), 2. fact(w — its running voter-stake
//!    total is decremented, NOT closed), 3. dest_kass(w), 4. stake_vault(w),
//! 5. rent_recipient(w == fact_vote.voter), 6. token program.
//!
//! # Fact-close ordering (no griefing)
//! `claim_fact` CLOSES the `Fact`, but `claim_fact_vote` must read the Fact's
//! disposition. So the submitter's claim runs LAST: each `claim_fact_vote`
//! decrements the Fact's `approve_stake`/`duplicate_stake` running total, and
//! `claim_fact` refuses to close while either is non-zero
//! ([`KassandraError::VotersOutstanding`]). No one can strand a voter by closing
//! the Fact early.
//!
//! # Instruction payload (after the 1-byte discriminant)
//! `oracle_nonce: u64 LE` (exactly 8 bytes) — re-derives + verifies the oracle
//! PDA signer seeds, identical to `settle_challenge`.

mod common;
mod fact;
mod fact_vote;
mod proposer;

pub use fact::claim_fact;
pub use fact_vote::claim_fact_vote;
pub use proposer::claim_proposer;
