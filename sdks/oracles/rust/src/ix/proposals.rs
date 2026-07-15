//! Proposal-round instruction builders (Ix 11–12).

use kassandra_oracles_program::instruction::Ix;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::build;
use crate::{SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID};

// ===================================================================== Ix 11
/// `Propose` (Ix 11) — register a categorical `option` with a KASS `bond`.
#[allow(clippy::too_many_arguments)]
pub fn propose(
    program_id: &Pubkey,
    oracle: Pubkey,
    proposer: Pubkey,
    authority: Pubkey,
    authority_kass: Pubkey,
    stake_vault: Pubkey,
    option: u8,
    bond: u64,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 9);
    data.push(Ix::Propose as u8);
    data.push(option);
    data.extend_from_slice(&bond.to_le_bytes());
    build(
        program_id,
        vec![
            AccountMeta::new(oracle, false),
            AccountMeta::new(proposer, false),
            AccountMeta::new(authority, true),
            AccountMeta::new(authority_kass, false),
            AccountMeta::new(stake_vault, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    )
}

// ===================================================================== Ix 12
/// `FinalizeProposals` (Ix 12) — close the proposal window. `tail` must be
/// EXACTLY `oracle.proposer_count` read-only Proposer accounts.
pub fn finalize_proposals(program_id: &Pubkey, oracle: Pubkey, tail: &[Pubkey]) -> Instruction {
    let mut accounts = Vec::with_capacity(1 + tail.len());
    accounts.push(AccountMeta::new(oracle, false));
    for p in tail {
        accounts.push(AccountMeta::new_readonly(*p, false));
    }
    build(program_id, accounts, vec![Ix::FinalizeProposals as u8])
}
