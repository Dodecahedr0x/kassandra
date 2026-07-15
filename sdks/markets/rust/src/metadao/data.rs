//! Arg encoders (discriminator ++ Borsh body) for the MetaDAO CPI wire format.

use super::{
    ADD_LIQUIDITY_DISC, CREATE_AMM_DISC, INITIALIZE_CONDITIONAL_VAULT_DISC, INITIALIZE_QUESTION_DISC,
    MERGE_TOKENS_DISC, REDEEM_TOKENS_DISC, SPLIT_TOKENS_DISC, SWAP_DISC, SwapType,
};
use solana_sdk::pubkey::Pubkey;

/// `initialize_question` data: `disc[8] ++ question_id[32] ++ oracle[32] ++ num_outcomes[1]`.
pub fn initialize_question_data(
    question_id: &[u8; 32],
    oracle_authority: &Pubkey,
    num_outcomes: u8,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(73);
    out.extend_from_slice(&INITIALIZE_QUESTION_DISC);
    out.extend_from_slice(question_id);
    out.extend_from_slice(oracle_authority.as_ref());
    out.push(num_outcomes);
    out
}

/// `initialize_conditional_vault` data — discriminator only.
pub fn initialize_conditional_vault_data() -> Vec<u8> {
    INITIALIZE_CONDITIONAL_VAULT_DISC.to_vec()
}

/// `split_tokens` data: `disc[8] ++ amount[8 LE]`.
pub fn split_tokens_data(amount: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&SPLIT_TOKENS_DISC);
    out.extend_from_slice(&amount.to_le_bytes());
    out
}

/// `merge_tokens` data: `disc[8] ++ amount[8 LE]`.
pub fn merge_tokens_data(amount: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&MERGE_TOKENS_DISC);
    out.extend_from_slice(&amount.to_le_bytes());
    out
}

/// `redeem_tokens` data — discriminator only (no args).
pub fn redeem_tokens_data() -> Vec<u8> {
    REDEEM_TOKENS_DISC.to_vec()
}

/// `create_amm` data: `disc[8] ++ twap_initial_observation[u128] ++
/// twap_max_observation_change_per_update[u128] ++ twap_start_delay_slots[u64]`.
pub fn create_amm_data(
    twap_initial_observation: u128,
    twap_max_change: u128,
    twap_start_delay: u64,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(40);
    out.extend_from_slice(&CREATE_AMM_DISC);
    out.extend_from_slice(&twap_initial_observation.to_le_bytes());
    out.extend_from_slice(&twap_max_change.to_le_bytes());
    out.extend_from_slice(&twap_start_delay.to_le_bytes());
    out
}

/// `add_liquidity` data: `disc[8] ++ quote_amount[u64] ++ max_base_amount[u64] ++
/// min_lp_tokens[u64]`.
pub fn add_liquidity_data(quote_amount: u64, max_base_amount: u64, min_lp_tokens: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(&ADD_LIQUIDITY_DISC);
    out.extend_from_slice(&quote_amount.to_le_bytes());
    out.extend_from_slice(&max_base_amount.to_le_bytes());
    out.extend_from_slice(&min_lp_tokens.to_le_bytes());
    out
}

/// `swap` data: `disc[8] ++ swap_type[u8] ++ input_amount[u64] ++
/// output_amount_min[u64]`.
pub fn swap_data(swap_type: SwapType, input_amount: u64, output_amount_min: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(25);
    out.extend_from_slice(&SWAP_DISC);
    out.push(swap_type as u8);
    out.extend_from_slice(&input_amount.to_le_bytes());
    out.extend_from_slice(&output_amount_min.to_le_bytes());
    out
}
