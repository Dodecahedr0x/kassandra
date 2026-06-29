//! Protocol-global constants.
//!
//! Values here are part of the program's economic/temporal contract. Keep them
//! centralized so tasks that reason about windows and thresholds share one
//! source of truth.

/// Duration (seconds) of a dispute phase window. When a phase is advanced, the
/// new `phase_ends_at` is set to `now + PHASE_WINDOW`.
pub const PHASE_WINDOW: i64 = 3600;

/// Protocol-global supermajority threshold (numerator) for fact approval.
///
/// A fact is agreed only if its approve-stake reaches this fraction of the
/// fixed `Oracle.dispute_bond_total`. Default 2/3 (supermajority).
pub const THRESHOLD_NUM: u64 = 2;
/// Protocol-global supermajority threshold (denominator) for fact approval.
pub const THRESHOLD_DEN: u64 = 3;

/// Fraction (numerator) of a proposer's bond slashed when they FLIP their value
/// at AI-claim time (submitted a `claim_option != original_option`). A flip is
/// penalized but not fatal: the proposer keeps a valid (flipped) claim that
/// still counts in the plurality, so they remain surviving. Default 1/2 (50%).
pub const FLIP_SLASH_NUM: u64 = 1;
/// Fraction (denominator) of the flip slash. See [`FLIP_SLASH_NUM`].
pub const FLIP_SLASH_DEN: u64 = 2;
