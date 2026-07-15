//! Program IDs, Anchor instruction discriminators, and PDA seed prefixes.

use pinocchio::address::Address as Pubkey;

// ─────────────────────────────────────────────────────────────────────────────
// Program IDs
// ─────────────────────────────────────────────────────────────────────────────

/// MetaDAO `conditional_vault` v0.4.0 (mainnet-beta).
pub const CONDITIONAL_VAULT_ID: Pubkey =
    Pubkey::from_str_const("VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg");

/// MetaDAO `amm` v0.4 (mainnet-beta).
pub const AMM_ID: Pubkey = Pubkey::from_str_const("AMMyu265tkBpRW21iGQxKGLaves3gKm2JcMUqfXNSpqD");

// ─────────────────────────────────────────────────────────────────────────────
// Anchor instruction discriminators — sha256("global:<name>")[..8]
// ─────────────────────────────────────────────────────────────────────────────

/// `conditional_vault::initialize_question`
pub const INITIALIZE_QUESTION: [u8; 8] = [0xf5, 0x97, 0x6a, 0xbc, 0x58, 0x2c, 0x41, 0xd4];
/// `conditional_vault::resolve_question`
pub const RESOLVE_QUESTION: [u8; 8] = [0x34, 0x20, 0xe0, 0xb3, 0xb4, 0x08, 0x00, 0xf6];
/// `conditional_vault::initialize_conditional_vault`
pub const INITIALIZE_CONDITIONAL_VAULT: [u8; 8] = [0x25, 0x58, 0xfa, 0xd4, 0x36, 0xda, 0xe3, 0xaf];
/// `conditional_vault::split_tokens`
pub const SPLIT_TOKENS: [u8; 8] = [0x4f, 0xc3, 0x74, 0x00, 0x8c, 0xb0, 0x49, 0xb3];
/// `conditional_vault::merge_tokens`
pub const MERGE_TOKENS: [u8; 8] = [0xe2, 0x59, 0xfb, 0x79, 0xe1, 0x82, 0xb4, 0x0e];
/// `conditional_vault::redeem_tokens`
pub const REDEEM_TOKENS: [u8; 8] = [0xf6, 0x62, 0x86, 0x29, 0x98, 0x21, 0x78, 0x45];

/// `amm::create_amm` — args (delayed-twap v0.4.1+) = `CreateAmmArgs {
/// twap_initial_observation: u128, twap_max_observation_change_per_update: u128,
/// twap_start_delay_slots: u64 }` (Borsh, 40 bytes). The base v0.4 build had only
/// the two u128s; the DEPLOYED mainnet binary requires the trailing u64.
pub const CREATE_AMM: [u8; 8] = [0xf2, 0x5b, 0x15, 0xaa, 0x05, 0x44, 0x7d, 0x40];
/// `amm::add_liquidity` — args = `AddLiquidityArgs { quote_amount: u64,
/// max_base_amount: u64, min_lp_tokens: u64 }`.
pub const ADD_LIQUIDITY: [u8; 8] = [0xb5, 0x9d, 0x59, 0x43, 0x8f, 0xb6, 0x34, 0x48];
/// `amm::remove_liquidity`.
pub const REMOVE_LIQUIDITY: [u8; 8] = [0x50, 0x55, 0xd1, 0x48, 0x18, 0xce, 0xb1, 0x6c];
/// `amm::swap` — args = `SwapArgs { swap_type: u8 (0=Buy,1=Sell), input_amount:
/// u64, output_amount_min: u64 }`.
pub const SWAP: [u8; 8] = [0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];
/// `amm::crank_that_twap` — folds the current price into the TWAP observation
/// (only once per `ONE_MINUTE_IN_SLOTS == 150` slots). No args; accounts =
/// `[amm(w), event_authority, amm_program]`.
pub const CRANK_THAT_TWAP: [u8; 8] = [0xdc, 0x64, 0x19, 0xf9, 0x00, 0x5c, 0xc3, 0xc1];

// ─────────────────────────────────────────────────────────────────────────────
// PDA seeds (from the conditional_vault source)
// ─────────────────────────────────────────────────────────────────────────────

/// `Question` PDA seed prefix.
pub const SEED_QUESTION: &[u8] = b"question";
/// `ConditionalVault` PDA seed prefix.
pub const SEED_CONDITIONAL_VAULT: &[u8] = b"conditional_vault";
/// Conditional-token mint PDA seed prefix.
pub const SEED_CONDITIONAL_TOKEN: &[u8] = b"conditional_token";
/// Anchor `#[event_cpi]` event-authority PDA seed.
pub const SEED_EVENT_AUTHORITY: &[u8] = b"__event_authority";
