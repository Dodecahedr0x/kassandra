//! Anchor instruction discriminators — sha256("global:<name>")[..8] — plus the
//! `SwapType` tag and the `Amm` account discriminator.

/// `conditional_vault::initialize_question`
pub const INITIALIZE_QUESTION_DISC: [u8; 8] = [0xf5, 0x97, 0x6a, 0xbc, 0x58, 0x2c, 0x41, 0xd4];
/// `conditional_vault::initialize_conditional_vault`
pub const INITIALIZE_CONDITIONAL_VAULT_DISC: [u8; 8] =
    [0x25, 0x58, 0xfa, 0xd4, 0x36, 0xda, 0xe3, 0xaf];
/// `conditional_vault::split_tokens`
pub const SPLIT_TOKENS_DISC: [u8; 8] = [0x4f, 0xc3, 0x74, 0x00, 0x8c, 0xb0, 0x49, 0xb3];
/// `conditional_vault::merge_tokens`
pub const MERGE_TOKENS_DISC: [u8; 8] = [0xe2, 0x59, 0xfb, 0x79, 0xe1, 0x82, 0xb4, 0x0e];
/// `conditional_vault::redeem_tokens`
pub const REDEEM_TOKENS_DISC: [u8; 8] = [0xf6, 0x62, 0x86, 0x29, 0x98, 0x21, 0x78, 0x45];
/// `conditional_vault::resolve_question`
pub const RESOLVE_QUESTION_DISC: [u8; 8] = [0x34, 0x20, 0xe0, 0xb3, 0xb4, 0x08, 0x00, 0xf6];
/// `amm::create_amm`
pub const CREATE_AMM_DISC: [u8; 8] = [0xf2, 0x5b, 0x15, 0xaa, 0x05, 0x44, 0x7d, 0x40];
/// `amm::add_liquidity`
pub const ADD_LIQUIDITY_DISC: [u8; 8] = [0xb5, 0x9d, 0x59, 0x43, 0x8f, 0xb6, 0x34, 0x48];
/// `amm::swap`
pub const SWAP_DISC: [u8; 8] = [0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];

/// `amm::SwapType` Borsh tag: `Buy` (quote→base) or `Sell` (base→quote).
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwapType {
    Buy = 0,
    Sell = 1,
}

/// `Amm` account discriminator (`sha256("account:Amm")[..8]`).
pub const AMM_ACCOUNT_DISCRIMINATOR: [u8; 8] = [0x8f, 0xf5, 0xc8, 0x11, 0x4a, 0xd6, 0xc4, 0x87];
