//! Protocol / oracle creation instruction builders (Ix 9–10).

use kassandra_oracles_program::instruction::Ix;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::build;
use crate::{SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID};

// ===================================================================== Ix 9
/// `InitProtocol` (Ix 9) — create the `[b"protocol"]` singleton.
pub fn init_protocol(
    program_id: &Pubkey,
    protocol: Pubkey,
    admin: Pubkey,
    kass_mint: Pubkey,
    usdc_mint: Pubkey,
) -> Instruction {
    build(
        program_id,
        vec![
            AccountMeta::new(protocol, false),
            AccountMeta::new(admin, true),
            AccountMeta::new_readonly(kass_mint, false),
            AccountMeta::new_readonly(usdc_mint, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        vec![Ix::InitProtocol as u8],
    )
}

// ===================================================================== Ix 10
/// `CreateOracle` (Ix 10). Derives the protocol, stake-vault, and mint-authority
/// PDAs internally. Payload order is nonce, options_count, deadline, twap_window
/// (NOT the account order). The subject now lives on-chain in `oracle_meta`.
#[allow(clippy::too_many_arguments)]
pub fn create_oracle(
    program_id: &Pubkey,
    nonce: u64,
    options_count: u8,
    deadline: i64,
    twap_window: i64,
    oracle: Pubkey,
    kass_mint: Pubkey,
    usdc_mint: Pubkey,
    creator: Pubkey,
    creator_kass: Pubkey,
) -> Instruction {
    let (protocol, _) = crate::pda::protocol(program_id);
    let (stake_vault, _) = crate::pda::stake_vault(program_id, &oracle);
    let (mint_authority, _) = crate::pda::mint_authority(program_id);

    let mut data = Vec::with_capacity(1 + 25);
    data.push(Ix::CreateOracle as u8);
    data.extend_from_slice(&nonce.to_le_bytes());
    data.push(options_count);
    data.extend_from_slice(&deadline.to_le_bytes());
    data.extend_from_slice(&twap_window.to_le_bytes());

    build(
        program_id,
        vec![
            AccountMeta::new(protocol, false),
            AccountMeta::new(oracle, false),
            AccountMeta::new(stake_vault, false),
            AccountMeta::new(creator, true),
            AccountMeta::new(kass_mint, false),
            AccountMeta::new_readonly(usdc_mint, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new(creator_kass, false),
            AccountMeta::new_readonly(mint_authority, false),
        ],
        data,
    )
}
