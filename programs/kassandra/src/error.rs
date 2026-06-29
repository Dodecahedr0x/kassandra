//! Program error codes surfaced to clients as `ProgramError::Custom(u32)`.
//!
//! Discriminants are a **stable public contract**: clients decode them from
//! `InstructionError::Custom(n)`, so existing values must never be renumbered.
//! New errors are APPENDED with the next free discriminant.

use pinocchio::program_error::ProgramError;

/// Custom error codes for the Kassandra dispute-core program.
///
/// Each variant maps to `ProgramError::Custom(discriminant)`.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KassandraError {
    /// Instruction is recognized but its processor is not implemented yet.
    NotImplemented = 0,
    /// The oracle is not in the phase this instruction requires.
    WrongPhase = 1,
    /// The current phase window has already closed (`now >= phase_ends_at`).
    WindowClosed = 2,
    /// The current phase window has not yet elapsed (`now < phase_ends_at`).
    WindowNotElapsed = 3,
    /// The signer is not authorized to perform this action.
    Unauthorized = 4,
    /// An account passed to the instruction is invalid (wrong owner, address,
    /// or contents).
    InvalidAccount = 5,
    /// A fact with this `content_hash` already exists for this oracle (the
    /// Fact PDA is already initialized).
    DuplicateFact = 6,
    /// A stake amount of zero was supplied where a positive stake is required
    /// (a zero-stake fact would pollute quorum for free).
    ZeroStake = 7,
    /// This voter has already voted on this fact (the FactVote PDA is already
    /// initialized): one vote per voter per fact.
    DuplicateVote = 8,
}

impl From<KassandraError> for ProgramError {
    fn from(e: KassandraError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
