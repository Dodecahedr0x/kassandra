//! Shared primitives: the `Pubkey` alias, on-chain sentinel constants, and the
//! `AccountType` / `Phase` discriminant enums.

/// 32-byte Solana public key. Aliases pinocchio's `Address` — a
/// `#[repr(transparent)]` newtype over `[u8; 32]` that is `Pod`/`Zeroable` (via
/// solana-address's `bytemuck` feature), so the zero-copy account structs below
/// keep the exact same byte layout while gaining typed key comparisons.
pub type Pubkey = pinocchio::address::Address;

/// `Proposer.claim_option` sentinel: no AI claim submitted yet.
pub const CLAIM_OPTION_NONE: u8 = 0xFF;
/// `FactVote.kind`: approve vote.
pub const VOTE_APPROVE: u8 = 0;
/// `FactVote.kind`: duplicate vote.
pub const VOTE_DUPLICATE: u8 = 1;

/// On-chain account-type discriminator. Stored as the FIRST byte of every Pod
/// account (each struct's `account_type` field) so processors can reject
/// type-confusion: an attacker cannot pass a `Fact` where an `Oracle` is
/// expected because the tag won't match. `Uninitialized` (0) is what a freshly
/// `CreateAccount`'d, zeroed account carries before it is stamped.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccountType {
    Uninitialized = 0,
    Oracle = 1,
    Proposer = 2,
    Fact = 3,
    FactVote = 4,
    AiClaim = 5,
    Market = 6,
    Protocol = 7,
    OracleMeta = 8,
}

impl AccountType {
    /// Encode this tag as its stored `u8` discriminant.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Lifecycle phase of an oracle dispute. Stored on-chain as a `u8`
/// discriminant (see [`Oracle::phase`]) to keep account structs `Pod`.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// RESERVED / UNUSED: `create_oracle` (H1) initializes oracles directly into
    /// [`Phase::Proposal`], so no live oracle is ever in `Created`. Kept for ABI
    /// stability (the discriminant must not be renumbered); do not remove.
    Created = 0,
    Proposal = 1,
    FactProposal = 2,
    FactVoting = 3,
    AiClaim = 4,
    Challenge = 5,
    FinalRecompute = 6,
    Resolved = 7,
    InvalidDeadend = 8,
}

impl Phase {
    /// Safely convert a stored `u8` discriminant back into a `Phase`.
    pub fn from_u8(x: u8) -> Option<Self> {
        match x {
            0 => Some(Phase::Created),
            1 => Some(Phase::Proposal),
            2 => Some(Phase::FactProposal),
            3 => Some(Phase::FactVoting),
            4 => Some(Phase::AiClaim),
            5 => Some(Phase::Challenge),
            6 => Some(Phase::FinalRecompute),
            7 => Some(Phase::Resolved),
            8 => Some(Phase::InvalidDeadend),
            _ => None,
        }
    }

    /// Encode this phase as its stored `u8` discriminant.
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}
