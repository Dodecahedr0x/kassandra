//! Fact-round instruction builders (Ix 0–2).

use kassandra_oracles_program::instruction::Ix;
use solana_instruction::AccountMeta;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::build;
use crate::{SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID};

// ===================================================================== Ix 0
/// `SubmitFact` (Ix 0) — post a candidate fact with a KASS stake.
#[allow(clippy::too_many_arguments)]
pub fn submit_fact(
    program_id: &Pubkey,
    oracle: Pubkey,
    fact: Pubkey,
    submitter: Pubkey,
    submitter_kass: Pubkey,
    stake_vault: Pubkey,
    content_hash: &[u8; 32],
    stake: u64,
    uri: &[u8],
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 42 + uri.len());
    data.push(Ix::SubmitFact as u8);
    data.extend_from_slice(content_hash);
    data.extend_from_slice(&stake.to_le_bytes());
    data.extend_from_slice(&(uri.len() as u16).to_le_bytes());
    data.extend_from_slice(uri);
    build(
        program_id,
        vec![
            AccountMeta::new(oracle, false),
            AccountMeta::new(fact, false),
            AccountMeta::new(submitter, true),
            AccountMeta::new(submitter_kass, false),
            AccountMeta::new(stake_vault, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    )
}

// ===================================================================== Ix 1
/// `VoteFact` (Ix 1) — approve (`kind = 0`) or mark-duplicate (`kind = 1`) a fact.
#[allow(clippy::too_many_arguments)]
pub fn vote_fact(
    program_id: &Pubkey,
    oracle: Pubkey,
    fact: Pubkey,
    fact_vote: Pubkey,
    voter: Pubkey,
    voter_kass: Pubkey,
    stake_vault: Pubkey,
    kind: u8,
    stake: u64,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 9);
    data.push(Ix::VoteFact as u8);
    data.push(kind);
    data.extend_from_slice(&stake.to_le_bytes());
    build(
        program_id,
        vec![
            AccountMeta::new(oracle, false),
            AccountMeta::new(fact, false),
            AccountMeta::new(fact_vote, false),
            AccountMeta::new(voter, true),
            AccountMeta::new(voter_kass, false),
            AccountMeta::new(stake_vault, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    )
}

// ===================================================================== Ix 2
/// `FinalizeFacts` (Ix 2) — incrementally settle the fact-voting phase. `tail`
/// is a non-empty writable subset of Facts (normal) or Proposers (no-facts
/// dead-end). Head burns via the oracle PDA (needs `nonce`).
pub fn finalize_facts(
    program_id: &Pubkey,
    oracle: Pubkey,
    kass_mint: Pubkey,
    stake_vault: Pubkey,
    nonce: u64,
    tail: &[Pubkey],
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 8);
    data.push(Ix::FinalizeFacts as u8);
    data.extend_from_slice(&nonce.to_le_bytes());
    let mut accounts = Vec::with_capacity(4 + tail.len());
    accounts.push(AccountMeta::new(oracle, false));
    accounts.push(AccountMeta::new(kass_mint, false));
    accounts.push(AccountMeta::new(stake_vault, false));
    accounts.push(AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false));
    for k in tail {
        accounts.push(AccountMeta::new(*k, false));
    }
    build(program_id, accounts, data)
}
