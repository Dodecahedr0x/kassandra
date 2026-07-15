//! Protocol governance instruction builders (Ix 13–16).

use kassandra_oracles_program::instruction::Ix;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::build;

// ===================================================================== Ix 13
/// `SetGovernance` (Ix 13) — record `dao_authority` + `kass_dao`. The `kass_dao`
/// account must equal the payload `kass_dao`.
pub fn set_governance(
    program_id: &Pubkey,
    protocol: Pubkey,
    authority: Pubkey,
    dao_authority: Pubkey,
    kass_dao: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 64);
    data.push(Ix::SetGovernance as u8);
    data.extend_from_slice(&dao_authority.to_bytes());
    data.extend_from_slice(&kass_dao.to_bytes());
    build(
        program_id,
        vec![
            AccountMeta::new(protocol, false),
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new_readonly(kass_dao, false),
        ],
        data,
    )
}

// ===================================================================== Ix 14
/// `SetConfig` (Ix 14) — overwrite the governable params. Payload = the 200-byte
/// packed [`crate::ConfigParams`].
pub fn set_config(
    program_id: &Pubkey,
    protocol: Pubkey,
    dao_authority: Pubkey,
    params: &crate::ConfigParams,
) -> Instruction {
    let mut data = Vec::with_capacity(1 + 200);
    data.push(Ix::SetConfig as u8);
    data.extend_from_slice(&params.to_payload());
    build(
        program_id,
        vec![
            AccountMeta::new(protocol, false),
            AccountMeta::new_readonly(dao_authority, true),
        ],
        data,
    )
}

// ===================================================================== Ix 15
/// `ResolveDeadend` (Ix 15) — DAO-gated resolution of a dead-ended oracle.
pub fn resolve_deadend(
    program_id: &Pubkey,
    protocol: Pubkey,
    oracle: Pubkey,
    dao_authority: Pubkey,
    option: u8,
) -> Instruction {
    build(
        program_id,
        vec![
            AccountMeta::new_readonly(protocol, false),
            AccountMeta::new(oracle, false),
            AccountMeta::new_readonly(dao_authority, true),
        ],
        vec![Ix::ResolveDeadend as u8, option],
    )
}

// ===================================================================== Ix 16
/// `KassPrice` (Ix 16) — read the governance-anchored KASS/USDC spot TWAP.
pub fn kass_price(program_id: &Pubkey, protocol: Pubkey, kass_dao: Pubkey) -> Instruction {
    build(
        program_id,
        vec![
            AccountMeta::new_readonly(protocol, false),
            AccountMeta::new_readonly(kass_dao, false),
        ],
        vec![Ix::KassPrice as u8],
    )
}
