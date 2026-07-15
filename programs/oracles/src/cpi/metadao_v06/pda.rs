//! Seed-slice assembly (host-runnable) and the SBF-only PDA derivations.

use pinocchio::address::Address as Pubkey;

use super::wire::{
    FUTARCHY_ID, SEED_DAO, SEED_EVENT_AUTHORITY, SEED_PROPOSAL, SQUADS_SEED_MULTISIG,
    SQUADS_SEED_PREFIX, SQUADS_SEED_VAULT, SQUADS_V4_ID,
};

// ─────────────────────────────────────────────────────────────────────────────
// Seed-slice assembly (host-runnable; `find_program_address` is SBF-only)
// ─────────────────────────────────────────────────────────────────────────────

/// futarchy `Dao` PDA seeds: `[b"dao", dao_creator, nonce_le[8]]`.
pub fn dao_seeds<'a>(dao_creator: &'a Pubkey, nonce_le: &'a [u8; 8]) -> [&'a [u8]; 3] {
    [SEED_DAO, dao_creator.as_ref(), nonce_le]
}

/// futarchy `Proposal` PDA seeds: `[b"proposal", squads_proposal]`.
pub fn proposal_seeds(squads_proposal: &Pubkey) -> [&[u8]; 2] {
    [SEED_PROPOSAL, squads_proposal.as_ref()]
}

/// Squads multisig PDA seeds: `[b"multisig", b"multisig", create_key]` where
/// `create_key` == the futarchy `Dao` PDA.
pub fn squads_multisig_seeds(dao: &Pubkey) -> [&[u8]; 3] {
    [SQUADS_SEED_PREFIX, SQUADS_SEED_MULTISIG, dao.as_ref()]
}

/// Squads vault (DAO execution authority) PDA seeds:
/// `[b"multisig", multisig, b"vault", vault_index_le[1]]`. The futarchy DAO uses
/// vault index 0.
pub fn squads_vault_seeds<'a>(multisig: &'a Pubkey, vault_index: &'a [u8; 1]) -> [&'a [u8]; 4] {
    [
        SQUADS_SEED_PREFIX,
        multisig.as_ref(),
        SQUADS_SEED_VAULT,
        vault_index,
    ]
}

/// futarchy `#[event_cpi]` event-authority PDA seeds: `[b"__event_authority"]`.
pub fn event_authority_seeds() -> [&'static [u8]; 1] {
    [SEED_EVENT_AUTHORITY]
}

// ─────────────────────────────────────────────────────────────────────────────
// PDA derivation (SBF-only — wrap the seed builders above)
// ─────────────────────────────────────────────────────────────────────────────

/// futarchy `Dao` PDA.
pub fn dao_pda(dao_creator: &Pubkey, nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&dao_seeds(dao_creator, &nonce.to_le_bytes()), &FUTARCHY_ID)
}

/// futarchy `Proposal` PDA.
pub fn proposal_pda(squads_proposal: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&proposal_seeds(squads_proposal), &FUTARCHY_ID)
}

/// Squads multisig PDA for a DAO (create_key == `dao`).
pub fn squads_multisig_pda(dao: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&squads_multisig_seeds(dao), &SQUADS_V4_ID)
}

/// Squads **vault** PDA (DAO execution authority) for `multisig` at `vault_index`.
/// This is the key Kassandra stores as `Protocol.dao_authority` and requires as
/// signer on `set_config` / `resolve_deadend`.
pub fn squads_vault_pda(multisig: &Pubkey, vault_index: u8) -> (Pubkey, u8) {
    Pubkey::find_program_address(&squads_vault_seeds(multisig, &[vault_index]), &SQUADS_V4_ID)
}

/// futarchy `#[event_cpi]` event-authority PDA.
pub fn event_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&event_authority_seeds(), &FUTARCHY_ID)
}
