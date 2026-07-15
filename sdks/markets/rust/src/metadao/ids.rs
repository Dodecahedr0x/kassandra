//! Program IDs for the MetaDAO CPI wire format.

use solana_sdk::{pubkey, pubkey::Pubkey};

/// MetaDAO `conditional_vault` v0.4.0 (mainnet-beta).
pub const CONDITIONAL_VAULT_ID: Pubkey = pubkey!("VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg");
/// MetaDAO `amm` v0.4.2 delayed-twap (mainnet-beta).
pub const AMM_ID: Pubkey = pubkey!("AMMyu265tkBpRW21iGQxKGLaves3gKm2JcMUqfXNSpqD");

/// SPL Token program.
pub const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
/// SPL Associated-Token-Account program.
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
