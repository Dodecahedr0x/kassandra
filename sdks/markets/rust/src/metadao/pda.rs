//! PDA seeds + derivers for the MetaDAO CPI wire format.

use solana_sdk::pubkey::Pubkey;

use super::{AMM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, CONDITIONAL_VAULT_ID, TOKEN_PROGRAM_ID};

const SEED_QUESTION: &[u8] = b"question";
const SEED_CONDITIONAL_VAULT: &[u8] = b"conditional_vault";
const SEED_CONDITIONAL_TOKEN: &[u8] = b"conditional_token";
const SEED_EVENT_AUTHORITY: &[u8] = b"__event_authority";
const SEED_AMM: &[u8] = b"amm__";
const SEED_AMM_LP_MINT: &[u8] = b"amm_lp_mint";

/// `Question` PDA: seeds `[b"question", question_id, oracle_authority, [num_outcomes]]`.
pub fn question(
    question_id: &[u8; 32],
    oracle_authority: &Pubkey,
    num_outcomes: u8,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SEED_QUESTION,
            question_id,
            oracle_authority.as_ref(),
            &[num_outcomes],
        ],
        &CONDITIONAL_VAULT_ID,
    )
}

/// `ConditionalVault` PDA: seeds `[b"conditional_vault", question, underlying_mint]`.
pub fn vault(question: &Pubkey, underlying_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            SEED_CONDITIONAL_VAULT,
            question.as_ref(),
            underlying_mint.as_ref(),
        ],
        &CONDITIONAL_VAULT_ID,
    )
}

/// Conditional-token mint PDA for `index`: seeds `[b"conditional_token", vault, [index]]`.
pub fn conditional_token_mint(vault: &Pubkey, index: u8) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[SEED_CONDITIONAL_TOKEN, vault.as_ref(), &[index]],
        &CONDITIONAL_VAULT_ID,
    )
}

/// `#[event_cpi]` event-authority PDA under `program_id`: seeds `[b"__event_authority"]`.
pub fn event_authority(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_EVENT_AUTHORITY], program_id)
}

/// `Amm` PDA: seeds `[b"amm__", base_mint, quote_mint]`.
pub fn amm(base_mint: &Pubkey, quote_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[SEED_AMM, base_mint.as_ref(), quote_mint.as_ref()],
        &AMM_ID,
    )
}

/// AMM LP-mint PDA: seeds `[b"amm_lp_mint", amm]`.
pub fn amm_lp_mint(amm: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SEED_AMM_LP_MINT, amm.as_ref()], &AMM_ID)
}

/// Associated token account for `owner`/`mint` (classic SPL Token program).
pub fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}
