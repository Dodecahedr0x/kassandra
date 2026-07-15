//! Claim / close / sweep instruction builders (Ix 17–22).

use kassandra_oracles_program::instruction::Ix;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::build;
use crate::TOKEN_PROGRAM_ID;

// ===================================================================== Ix 17
/// `ClaimProposer` (Ix 17) — claim-and-close one proposer after the oracle is terminal.
pub fn claim_proposer(
    program_id: &Pubkey,
    oracle: Pubkey,
    nonce: u64,
    proposer: Pubkey,
    dest_kass: Pubkey,
    stake_vault: Pubkey,
    rent_recipient: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 8);
    data.push(Ix::ClaimProposer as u8);
    data.extend_from_slice(&nonce.to_le_bytes());
    build(
        program_id,
        vec![
            AccountMeta::new_readonly(oracle, false),
            AccountMeta::new(proposer, false),
            AccountMeta::new(dest_kass, false),
            AccountMeta::new(stake_vault, false),
            AccountMeta::new(rent_recipient, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data,
    )
}

// ===================================================================== Ix 18
/// `ClaimFact` (Ix 18) — claim-and-close one fact submitter.
pub fn claim_fact(
    program_id: &Pubkey,
    oracle: Pubkey,
    nonce: u64,
    fact: Pubkey,
    dest_kass: Pubkey,
    stake_vault: Pubkey,
    rent_recipient: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 8);
    data.push(Ix::ClaimFact as u8);
    data.extend_from_slice(&nonce.to_le_bytes());
    build(
        program_id,
        vec![
            AccountMeta::new_readonly(oracle, false),
            AccountMeta::new(fact, false),
            AccountMeta::new(dest_kass, false),
            AccountMeta::new(stake_vault, false),
            AccountMeta::new(rent_recipient, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data,
    )
}

// ===================================================================== Ix 19
/// `ClaimFactVote` (Ix 19) — claim-and-close one fact vote. `fact` (index 2) is
/// writable: its running voter-stake total is decremented (the fact is NOT closed).
#[allow(clippy::too_many_arguments)]
pub fn claim_fact_vote(
    program_id: &Pubkey,
    oracle: Pubkey,
    nonce: u64,
    fact_vote: Pubkey,
    fact: Pubkey,
    dest_kass: Pubkey,
    stake_vault: Pubkey,
    rent_recipient: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 8);
    data.push(Ix::ClaimFactVote as u8);
    data.extend_from_slice(&nonce.to_le_bytes());
    build(
        program_id,
        vec![
            AccountMeta::new_readonly(oracle, false),
            AccountMeta::new(fact_vote, false),
            AccountMeta::new(fact, false),
            AccountMeta::new(dest_kass, false),
            AccountMeta::new(stake_vault, false),
            AccountMeta::new(rent_recipient, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data,
    )
}

// ===================================================================== Ix 20
/// `CloseAiClaim` (Ix 20) — rent-reclaim close of an `AiClaim`. Empty payload.
pub fn close_ai_claim(
    program_id: &Pubkey,
    oracle: Pubkey,
    ai_claim: Pubkey,
    rent_recipient: Pubkey,
) -> Instruction {
    build(
        program_id,
        vec![
            AccountMeta::new_readonly(oracle, false),
            AccountMeta::new(ai_claim, false),
            AccountMeta::new(rent_recipient, false),
        ],
        vec![Ix::CloseAiClaim as u8],
    )
}

// ===================================================================== Ix 21
/// `CloseMarket` (Ix 21) — rent-reclaim close of a settled `Market` + its escrow.
pub fn close_market(
    program_id: &Pubkey,
    oracle: Pubkey,
    nonce: u64,
    market: Pubkey,
    challenger_usdc_vault: Pubkey,
    rent_recipient: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 8);
    data.push(Ix::CloseMarket as u8);
    data.extend_from_slice(&nonce.to_le_bytes());
    build(
        program_id,
        vec![
            AccountMeta::new_readonly(oracle, false),
            AccountMeta::new(market, false),
            AccountMeta::new(challenger_usdc_vault, false),
            AccountMeta::new(rent_recipient, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data,
    )
}

// ===================================================================== Ix 22
/// `SweepOracle` (Ix 22) — grace-gated dust sweep + terminal closure.
pub fn sweep_oracle(
    program_id: &Pubkey,
    oracle: Pubkey,
    nonce: u64,
    stake_vault: Pubkey,
    protocol: Pubkey,
    dao_treasury: Pubkey,
    creator: Pubkey,
    // The companion oracle_meta PDA, closed alongside the oracle (rent → creator).
    // `None` for an oracle that has no metadata (the close is skipped).
    oracle_meta: Option<Pubkey>,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 8);
    data.push(Ix::SweepOracle as u8);
    data.extend_from_slice(&nonce.to_le_bytes());
    let mut accounts = vec![
        AccountMeta::new(oracle, false),
        AccountMeta::new(stake_vault, false),
        AccountMeta::new_readonly(protocol, false),
        AccountMeta::new(dao_treasury, false),
        AccountMeta::new(creator, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ];
    if let Some(meta) = oracle_meta {
        accounts.push(AccountMeta::new(meta, false));
    }
    build(program_id, accounts, data)
}
