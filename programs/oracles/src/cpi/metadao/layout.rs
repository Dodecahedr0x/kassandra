//! Account-layout byte offsets, little-endian field readers, and the shared
//! `Amm` binding check.

use crate::error::KassandraError;
use pinocchio::{
    account::AccountView as AccountInfo, address::Address as Pubkey, error::ProgramError,
    ProgramResult,
};

use super::wire::AMM_ID;

// ─────────────────────────────────────────────────────────────────────────────
// Account layout byte offsets (single source of truth — verified against the
// deployed v0.4.0 source `metaDAOproject/programs`, declare_id! == VLTX1…)
// ─────────────────────────────────────────────────────────────────────────────
//
// Both `Question` and `ConditionalVault` carry the 8-byte Anchor account
// discriminator first, so every field offset below is `8 + <borsh offset>`.
// Task 10 (`open_challenge`) and Task 11 (`settle_challenge`) read these.

/// `Question.oracle: Pubkey` — byte offset (after the 8-byte Anchor disc).
pub const QUESTION_ORACLE_OFFSET: usize = 40;
/// `Question.payout_numerators: Vec<u32>` length-prefix offset. At
/// `initialize_question` the Vec is `vec![0; num_outcomes]`, so this u32 LE
/// length equals `num_outcomes`.
pub const QUESTION_NUM_OUTCOMES_LEN_OFFSET: usize = 72;
/// `ConditionalVault.question: Pubkey` — byte offset.
pub const VAULT_QUESTION_OFFSET: usize = 8;
/// `ConditionalVault.underlying_token_mint: Pubkey` — byte offset.
pub const VAULT_UNDERLYING_MINT_OFFSET: usize = 40;
/// `ConditionalVault.underlying_token_account: Pubkey` — byte offset.
pub const VAULT_UNDERLYING_ACCOUNT_OFFSET: usize = 72;

// ─────────────────────────────────────────────────────────────────────────────
// `amm` v0.4.x `Amm` account layout (verified against the source
// `metaDAOproject/programs`, `programs/amm/src/state/amm.rs`, `declare_id! ==
// AMMyu…`). The DEPLOYED mainnet binary is the "delayed-twap" v0.4.1/v0.4.2
// build (tags `delayed-twap-v0.4.1` / `proposal-duration-v0.4.2`), which added a
// `TwapOracle.start_delay_slots: u64` field AFTER `initial_observation` (and a
// `CreateAmmArgs.twap_start_delay_slots`). That new field sits *after* every
// field settle_challenge reads, so the offsets below are identical to the base
// v0.4 layout; only `seq_num` shifted (227 → unread).
// ─────────────────────────────────────────────────────────────────────────────
//
// The `Amm` account is an Anchor `#[account]` (8-byte disc first) Borsh-encoded
// (sequential, little-endian, NO alignment padding). Field order:
//
//   disc[8] | bump:u8 @8 | created_at_slot:u64 @9 | lp_mint:Pubkey @17
//   | base_mint:Pubkey @49 | quote_mint:Pubkey @81 | base_mint_decimals:u8 @113
//   | quote_mint_decimals:u8 @114 | base_amount:u64 @115 | quote_amount:u64 @123
//   | oracle: TwapOracle @131 { last_updated_slot:u64 @131, last_price:u128 @139,
//       last_observation:u128 @155, aggregator:u128 @171,
//       max_observation_change_per_update:u128 @187, initial_observation:u128 @203,
//       start_delay_slots:u64 @219 }   | seq_num:u64 @227
//
// `get_twap()` in the v0.4.2 source computes
//   `aggregator / (last_updated_slot - (created_at_slot + start_delay_slots))`
// — a slot-weighted average of the quote/base price (scaled by PRICE_SCALE =
// 1e12). settle_challenge reads exactly those four fields and mirrors that math.

/// Anchor account discriminator for `Amm` (`sha256("account:Amm")[..8]`). The
/// first 8 bytes of every `Amm` account; checked in settle as defense-in-depth
/// on top of the conditional mint-pair binding.
pub const AMM_ACCOUNT_DISCRIMINATOR: [u8; 8] = [0x8f, 0xf5, 0xc8, 0x11, 0x4a, 0xd6, 0xc4, 0x87];

/// `Amm.created_at_slot: u64` — byte offset.
pub const AMM_CREATED_AT_SLOT_OFFSET: usize = 9;
/// `Amm.base_mint: Pubkey` — byte offset.
pub const AMM_BASE_MINT_OFFSET: usize = 49;
/// `Amm.quote_mint: Pubkey` — byte offset.
pub const AMM_QUOTE_MINT_OFFSET: usize = 81;
/// `Amm.oracle.last_updated_slot: u64` — byte offset.
pub const AMM_LAST_UPDATED_SLOT_OFFSET: usize = 131;
/// `Amm.oracle.aggregator: u128` — byte offset.
pub const AMM_AGGREGATOR_OFFSET: usize = 171;
/// `Amm.oracle.start_delay_slots: u64` — byte offset (v0.4.1+ delayed-twap).
pub const AMM_START_DELAY_SLOTS_OFFSET: usize = 219;
/// Smallest `Amm` account data length that covers every field settle reads
/// (`start_delay_slots` end). The real account is larger (`8 +
/// size_of::<Amm>()`).
pub const AMM_MIN_LEN: usize = AMM_START_DELAY_SLOTS_OFFSET + 8;

// ─────────────────────────────────────────────────────────────────────────────
// Little-endian field readers (single source of truth, co-located with the
// offset consts). Shared by `open_challenge` and `settle_challenge` so the two
// processors decode MetaDAO account fields the same way; out-of-bounds reads map
// to `KassandraError::InvalidAccount`.
// ─────────────────────────────────────────────────────────────────────────────

/// Verify `amm` is a bound MetaDAO v0.4 `Amm` account for a specific conditional
/// pair: owned by the AMM program, long enough, carrying the `Amm` Anchor
/// discriminator, and whose recorded base/quote mints are EXACTLY
/// `expected_base`/`expected_quote` (this market's conditional (KASS, USDC) mint
/// pair for one outcome).
///
/// Shared by BOTH `open_challenge` and `settle_challenge`. Binding at open is
/// load-bearing: a `Market` recorded with an AMM that can't bind here could
/// never settle (`settle_challenge` pins to the RECORDED address), so
/// `open_challenge_count` would never return to 0, `finalize_oracle` would be
/// blocked forever, and every stake in the oracle would be permanently locked.
pub fn assert_amm_bound(
    amm: &AccountInfo,
    expected_base: &Pubkey,
    expected_quote: &Pubkey,
) -> ProgramResult {
    if !amm.owned_by(&AMM_ID) {
        return Err(KassandraError::InvalidAccount.into());
    }
    let data = amm.try_borrow()?;
    if data.len() < AMM_MIN_LEN || data[..8] != AMM_ACCOUNT_DISCRIMINATOR {
        return Err(KassandraError::InvalidAccount.into());
    }
    let base_mint = read_pubkey(&data, AMM_BASE_MINT_OFFSET)?;
    let quote_mint = read_pubkey(&data, AMM_QUOTE_MINT_OFFSET)?;
    if &base_mint != expected_base || &quote_mint != expected_quote {
        return Err(KassandraError::InvalidAccount.into());
    }
    Ok(())
}

/// Read a 32-byte pubkey out of `data` at byte `off`, or `InvalidAccount`.
pub fn read_pubkey(data: &[u8], off: usize) -> Result<Pubkey, ProgramError> {
    data.get(off..off + 32)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| KassandraError::InvalidAccount.into())
}

/// Read a little-endian `u32` out of `data` at byte `off`, or `InvalidAccount`.
pub fn read_u32(data: &[u8], off: usize) -> Result<u32, ProgramError> {
    data.get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| KassandraError::InvalidAccount.into())
}

/// Read a little-endian `u64` out of `data` at byte `off`, or `InvalidAccount`.
pub fn read_u64(data: &[u8], off: usize) -> Result<u64, ProgramError> {
    data.get(off..off + 8)
        .and_then(|s| s.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or_else(|| KassandraError::InvalidAccount.into())
}

/// Read a little-endian `u128` out of `data` at byte `off`, or `InvalidAccount`.
pub fn read_u128(data: &[u8], off: usize) -> Result<u128, ProgramError> {
    data.get(off..off + 16)
        .and_then(|s| s.try_into().ok())
        .map(u128::from_le_bytes)
        .ok_or_else(|| KassandraError::InvalidAccount.into())
}
