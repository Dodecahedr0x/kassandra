//! Protocol-global constants.
//!
//! Values here are part of the program's economic/temporal contract. Keep them
//! centralized so tasks that reason about windows and thresholds share one
//! source of truth.

/// Duration (seconds) of a dispute phase window. When a phase is advanced, the
/// new `phase_ends_at` is set to `now + PHASE_WINDOW`.
pub const PHASE_WINDOW: i64 = 3600;

/// Duration (seconds) of the proposal-registration window. `create_oracle` sets
/// the new oracle's `phase_ends_at = deadline + PROPOSAL_WINDOW`: proposals open
/// at the `deadline` and the window runs for this long afterward.
pub const PROPOSAL_WINDOW: i64 = 3600;

/// Upper bound on an oracle's proposer set, set to a realistic single-transaction
/// account-lock budget (Solana caps a tx at 64 account locks; `finalize_oracle`
/// also locks the oracle + program + fee payer, leaving ~60 read-only proposer
/// slots).
///
/// CONTRACT: this is the DEFENSIVE backstop that keeps `finalize_oracle`'s fixed
/// `votes` buffer from overflowing — AND the liveness guarantee enforced at
/// registration. The `propose` processor caps `proposer_count` at or below this
/// so the one-shot `finalize_oracle` always fits one transaction; otherwise an
/// oversized set would brick the oracle in [`Phase::Challenge`]. Shared by
/// `propose` and `finalize_oracle` so both reason about one constant.
pub const MAX_PROPOSERS: u16 = 60;

/// Protocol-global supermajority threshold (numerator) for fact approval.
///
/// A fact is agreed only if its approve-stake reaches this fraction of the
/// fixed `Oracle.dispute_bond_total`. Default 2/3 (supermajority).
pub const THRESHOLD_NUM: u64 = 2;
/// Protocol-global supermajority threshold (denominator) for fact approval.
pub const THRESHOLD_DEN: u64 = 3;

/// Market slash-trigger margin (numerator). A challenged claim is DISQUALIFIED
/// only if its decision-market `fail` TWAP exceeds its `pass` TWAP by at least
/// this fraction: `fail_twap > pass_twap * (1 + MARKET_THRESHOLD_NUM /
/// MARKET_THRESHOLD_DEN)`. Implemented overflow-safely in `u128` as
/// `fail_twap * DEN > pass_twap * (DEN + NUM)`. This is the protocol-global
/// "fail > pass + threshold" of design §6 / invariant §9.8, expressed as a
/// RELATIVE margin (robust across markets with different price scales).
///
/// Default 1/10 (fail must beat pass by at least 10%): a margin wide enough that
/// ordinary two-sided trading noise does not flip an honest claim, yet narrow
/// enough that a genuine fraud belief (fail bid up, pass bid down) crosses it.
/// SEPARATE from the fact-quorum [`THRESHOLD_NUM`]/[`THRESHOLD_DEN`].
pub const MARKET_THRESHOLD_NUM: u128 = 1;
/// Market slash-trigger margin (denominator). See [`MARKET_THRESHOLD_NUM`].
pub const MARKET_THRESHOLD_DEN: u128 = 10;

// ---------------------------------------------------------------------------
// Dynamic creation fee (KASS, burned) — Task H2 / design §8.
// ---------------------------------------------------------------------------
//
// The oracle-creation fee is paid in KASS and BURNED. It is proportional to an
// exponentially-decaying moving average ("EMA") of recent creation activity:
//
//   fee = FEE_PER_EMA_UNIT * (decayed_fee_ema / FEE_EMA_SCALE)
//
// `Protocol.fee_ema` is a fixed-point accumulator scaled by [`FEE_EMA_SCALE`]
// (so `fee_ema == FEE_EMA_SCALE` means "1.0 creation units of recent activity").
// On every creation we (1) decay the stored EMA toward 0 by the time elapsed
// since the last creation, (2) charge a fee proportional to that decayed value,
// then (3) bump the EMA by [`FEE_EMA_INCREMENT`] and stamp `last_creation_unix`.
//
// Consequences (design §8 "fee monotonicity"):
//   * Genesis: `fee_ema == 0` → decayed 0 → fee 0 (free bootstrap).
//   * Demand: rapid creations stack [`FEE_EMA_INCREMENT`] faster than decay can
//     erase it → EMA grows → fee grows.
//   * Idle: no creations → the EMA decays exponentially toward 0 → fee shrinks
//     back to 0.
// The fee is never negative and moves only as a function of the creation-rate
// EMA. All fee math is done in `u128` intermediates and is overflow-safe.

/// Fixed-point scale for [`crate::state::Protocol::fee_ema`]. `fee_ema` of this
/// value represents 1.0 "creation units" of recent activity.
pub const FEE_EMA_SCALE: u128 = 1_000_000_000;

/// Half-life (seconds) of the activity EMA: after this much idle time the EMA
/// (and thus the fee) halves. 1 day. Governance-tunable.
pub const FEE_EMA_HALFLIFE_SECS: i64 = 86_400;

/// Scaled EMA bump added per oracle creation: exactly one "creation unit"
/// (`1.0 * FEE_EMA_SCALE`). Each creation adds this to the (decayed) EMA.
pub const FEE_EMA_INCREMENT: u64 = FEE_EMA_SCALE as u64;

/// KASS base units charged per 1.0 of EMA activity (i.e. per `FEE_EMA_SCALE` of
/// `fee_ema`). KASS has 9 decimals, so this is 1 KASS per unit of EMA.
/// Governance-tunable.
pub const FEE_PER_EMA_UNIT: u64 = 1_000_000_000;

/// Fraction (numerator) of a proposer's bond slashed when they FLIP their value
/// at AI-claim time (submitted a `claim_option != original_option`). A flip is
/// penalized but not fatal: the proposer keeps a valid (flipped) claim that
/// still counts in the plurality, so they remain surviving. Default 1/2 (50%).
pub const FLIP_SLASH_NUM: u64 = 1;
/// Fraction (denominator) of the flip slash. See [`FLIP_SLASH_NUM`].
pub const FLIP_SLASH_DEN: u64 = 2;
