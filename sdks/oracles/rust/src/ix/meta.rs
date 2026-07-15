//! Oracle metadata instruction builder (Ix 23).

use kassandra_oracles_program::instruction::Ix;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::build;
use crate::SYSTEM_PROGRAM_ID;

// ===================================================================== Ix 23
/// `WriteOracleMeta` (Ix 23). Writes the companion `[b"oracle_meta", oracle]`
/// PDA — the plaintext `subject` + option `labels` + `uri`/`uri_hash`. The body
/// is length-prefixed (`subject_len u16 ++ subject ++ options_count u8 ++
/// [option_len u16 ++ option]* ++ uri_len u16 ++ uri ++ uri_hash[32]`); the
/// account is sized to fit and is write-once, gated to the oracle's creator.
#[allow(clippy::too_many_arguments)]
pub fn write_oracle_meta(
    program_id: &Pubkey,
    oracle: Pubkey,
    creator: Pubkey,
    subject: &str,
    options: &[&str],
    uri: &str,
    uri_hash: &[u8; 32],
) -> Instruction {
    let (meta, _) = crate::pda::oracle_meta(program_id, &oracle);

    let mut data = Vec::new();
    data.push(Ix::WriteOracleMeta as u8);
    data.extend_from_slice(&(subject.len() as u16).to_le_bytes());
    data.extend_from_slice(subject.as_bytes());
    data.push(options.len() as u8);
    for o in options {
        data.extend_from_slice(&(o.len() as u16).to_le_bytes());
        data.extend_from_slice(o.as_bytes());
    }
    data.extend_from_slice(&(uri.len() as u16).to_le_bytes());
    data.extend_from_slice(uri.as_bytes());
    data.extend_from_slice(uri_hash);

    build(
        program_id,
        vec![
            AccountMeta::new(creator, true),
            AccountMeta::new_readonly(oracle, false),
            AccountMeta::new(meta, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    )
}
