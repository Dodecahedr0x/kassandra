//! AI-claim submission instruction builders (Ix 3).

use kassandra_oracles_program::instruction::Ix;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::build;
use crate::SYSTEM_PROGRAM_ID;

// ===================================================================== Ix 3
/// `SubmitAiClaim` (Ix 3) — resubmit a value + AI-claim metadata over the agreed
/// facts. Assembles the 97-byte payload from its components.
#[allow(clippy::too_many_arguments)]
pub fn submit_ai_claim(
    program_id: &Pubkey,
    oracle: Pubkey,
    proposer: Pubkey,
    ai_claim: Pubkey,
    authority: Pubkey,
    model_id: &[u8; 32],
    params_hash: &[u8; 32],
    io_hash: &[u8; 32],
    option: u8,
) -> Instruction {
    let mut payload = [0u8; 97];
    payload[0..32].copy_from_slice(model_id);
    payload[32..64].copy_from_slice(params_hash);
    payload[64..96].copy_from_slice(io_hash);
    payload[96] = option;
    submit_ai_claim_raw(program_id, oracle, proposer, ai_claim, authority, &payload)
}

/// `SubmitAiClaim` (Ix 3) from a pre-computed 97-byte payload
/// (`model_id[32] ++ params_hash[32] ++ io_hash[32] ++ option[1]`). Used by the
/// runner, which passes the exact bytes it emitted as metadata so the submitted
/// claim can never diverge from the emitted claim.
pub fn submit_ai_claim_raw(
    program_id: &Pubkey,
    oracle: Pubkey,
    proposer: Pubkey,
    ai_claim: Pubkey,
    authority: Pubkey,
    payload: &[u8; 97],
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 97);
    data.push(Ix::SubmitAiClaim as u8);
    data.extend_from_slice(payload);
    build(
        program_id,
        vec![
            AccountMeta::new(oracle, false),
            AccountMeta::new(proposer, false),
            AccountMeta::new(ai_claim, false),
            AccountMeta::new(authority, true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    )
}
