//! Protocol-global constants.
//!
//! Values here are part of the program's economic/temporal contract. Keep them
//! centralized so tasks that reason about windows and thresholds share one
//! source of truth.

/// Duration (seconds) of a dispute phase window. When a phase is advanced, the
/// new `phase_ends_at` is set to `now + PHASE_WINDOW`.
pub const PHASE_WINDOW: i64 = 3600;
