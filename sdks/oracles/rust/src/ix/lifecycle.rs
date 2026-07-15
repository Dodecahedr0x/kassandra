//! Oracle lifecycle / round-finalization instruction builders (Ix 6–8).

use kassandra_oracles_program::instruction::Ix;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::build;
use crate::TOKEN_PROGRAM_ID;

// ===================================================================== Ix 6
/// `FinalizeOracle` (Ix 6) — compute the final plurality. `tail` must be EXACTLY
/// `oracle.proposer_count` read-only Proposer accounts (one-shot).
pub fn finalize_oracle(
    program_id: &Pubkey,
    oracle: Pubkey,
    kass_mint: Pubkey,
    stake_vault: Pubkey,
    nonce: u64,
    tail: &[Pubkey],
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 8);
    data.push(Ix::FinalizeOracle as u8);
    data.extend_from_slice(&nonce.to_le_bytes());
    let mut accounts = Vec::with_capacity(4 + tail.len());
    accounts.push(AccountMeta::new(oracle, false));
    accounts.push(AccountMeta::new(kass_mint, false));
    accounts.push(AccountMeta::new(stake_vault, false));
    accounts.push(AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false));
    for k in tail {
        accounts.push(AccountMeta::new_readonly(*k, false));
    }
    build(program_id, accounts, data)
}

// ===================================================================== Ix 7
/// `AdvancePhase` (Ix 7) — permissionless `FactProposal -> FactVoting` freeze.
pub fn advance_phase(program_id: &Pubkey, oracle: Pubkey) -> Instruction {
    build(
        program_id,
        vec![AccountMeta::new(oracle, false)],
        vec![Ix::AdvancePhase as u8],
    )
}

// ===================================================================== Ix 8
/// `FinalizeAiClaims` (Ix 8) — incrementally settle the AI-claim round. `tail`
/// is a non-empty writable subset of this oracle's Proposers.
pub fn finalize_ai_claims(program_id: &Pubkey, oracle: Pubkey, tail: &[Pubkey]) -> Instruction {
    let mut accounts = Vec::with_capacity(1 + tail.len());
    accounts.push(AccountMeta::new(oracle, false));
    for k in tail {
        accounts.push(AccountMeta::new(*k, false));
    }
    build(program_id, accounts, vec![Ix::FinalizeAiClaims as u8])
}
