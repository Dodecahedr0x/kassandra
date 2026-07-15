//! Account LAYOUTS, byte offsets, little-endian readers, and the futarchy spot
//! TWAP primitive.

use crate::error::KassandraError;
use pinocchio::{address::Address as Pubkey, error::ProgramError};

// ─────────────────────────────────────────────────────────────────────────────
// Account LAYOUTS (verified against metaDAOproject/programs @ v0.6.0 source)
// ─────────────────────────────────────────────────────────────────────────────
//
// ## `Dao` (futarchy, `#[account] #[derive(InitSpace)]`, programs/futarchy/src/
// state/dao.rs). Borsh field order (8-byte Anchor disc first):
//
//   disc[8]
//   amm: FutarchyAmm {
//       state: PoolState   <-- ENUM (variable length!), see below
//       total_liquidity: u128
//       base_mint: Pubkey
//       quote_mint: Pubkey
//       amm_base_vault: Pubkey
//       amm_quote_vault: Pubkey
//   }
//   nonce: u64
//   dao_creator: Pubkey
//   pda_bump: u8
//   squads_multisig: Pubkey
//   squads_multisig_vault: Pubkey   <-- the DAO execution authority (also a PDA)
//   base_mint: Pubkey
//   quote_mint: Pubkey
//   proposal_count: u32
//   pass_threshold_bps: u16
//   seconds_per_proposal: u32
//   twap_initial_observation: u128
//   twap_max_observation_change_per_update: u128
//   twap_start_delay_seconds: u32
//   min_quote_futarchic_liquidity: u64
//   min_base_futarchic_liquidity: u64
//   base_to_stake: u64
//   seq_num: u64
//   initial_spending_limit: Option<InitialSpendingLimit>
//
// CAUTION: `amm.state` is a Borsh enum `PoolState{ Spot{spot:Pool},
// Futarchy{spot,pass,fail:Pool} }`. Its SERIALIZED length depends on the variant
// (Spot = 1 + 1*Pool, Futarchy = 1 + 3*Pool), so EVERY `Dao` field after
// `amm.state` lives at a VARIABLE byte offset. They cannot be read with a fixed
// const; you must Borsh-decode the enum (or skip past it using the live variant
// tag). `Pool` borsh-serializes to 132 bytes (TwapOracle 100 + 4×u64 32). So:
//   - Spot  DAO: fields after state start at 8 + 1 + 132          = 141
//   - Futar DAO: fields after state start at 8 + 1 + 3*132        = 405
// `squads_multisig_vault` is then at +(8 nonce +32 creator +1 bump +32 multisig)
// = +73 from there (Spot: 214, Futarchy: 478). F1 should store the vault key
// directly in Kassandra's `Protocol` at bootstrap rather than re-derive it from
// `Dao` bytes, precisely because of this variable offset.
//
// ## Futarchy spot TWAP (the F5 `kass_price` source). The spot `Pool` is the
// FIRST payload element of BOTH PoolState variants, so its offsets ARE fixed
// regardless of variant:
//
//   byte 8  : PoolState enum tag (0 = Spot, 1 = Futarchy)
//   byte 9  : spot Pool starts == oracle (TwapOracle) starts
//             aggregator                          u128 @  9
//             last_updated_timestamp              i64  @ 25
//             created_at_timestamp                i64  @ 33
//             last_price                          u128 @ 41
//             last_observation                    u128 @ 57
//             max_observation_change_per_update   u128 @ 73
//             initial_observation                 u128 @ 89
//             start_delay_seconds                 u32  @105
//   byte 109: quote_reserves u64, base_reserves u64 @117, … (spot Pool tail)
//
// get_twap() (futarchy source) =
//     aggregator / (last_updated_timestamp - (created_at_timestamp + start_delay_seconds))
// requiring aggregator != 0 and last_updated_timestamp > start. The quotient is a
// price = quote_units_per_base * 1e12 (PRICE_SCALE); UI price further adjusts for
// base/quote decimals. [`futarchy_spot_twap`] mirrors this exactly.

/// PoolState enum-tag byte in a `Dao` account (0 = Spot, 1 = Futarchy).
pub const DAO_POOLSTATE_TAG_OFFSET: usize = 8;
/// Spot `Pool` start (== spot TwapOracle start) inside a `Dao` account.
pub const DAO_SPOT_POOL_OFFSET: usize = 9;
/// `TwapOracle.aggregator: u128` (spot) — byte offset.
pub const DAO_SPOT_AGGREGATOR_OFFSET: usize = 9;
/// `TwapOracle.last_updated_timestamp: i64` (spot) — byte offset.
pub const DAO_SPOT_LAST_UPDATED_TS_OFFSET: usize = 25;
/// `TwapOracle.created_at_timestamp: i64` (spot) — byte offset.
pub const DAO_SPOT_CREATED_AT_TS_OFFSET: usize = 33;
/// `TwapOracle.last_price: u128` (spot) — byte offset.
pub const DAO_SPOT_LAST_PRICE_OFFSET: usize = 41;
/// `TwapOracle.start_delay_seconds: u32` (spot) — byte offset.
pub const DAO_SPOT_START_DELAY_SECONDS_OFFSET: usize = 105;
/// Serialized size of one futarchy `Pool` (TwapOracle 100 + 4×u64 32).
pub const FUTARCHY_POOL_LEN: usize = 132;
/// Smallest `Dao` data length covering the spot TWAP fields.
pub const DAO_SPOT_TWAP_MIN_LEN: usize = DAO_SPOT_POOL_OFFSET + FUTARCHY_POOL_LEN;

// ## `Proposal` (futarchy, programs/futarchy/src/state/proposal.rs). Borsh order:
//
//   disc[8]
//   number: u32                 @  8
//   proposer: Pubkey            @ 12
//   timestamp_enqueued: i64     @ 44
//   state: ProposalState        @ 52   <-- ENUM (variable): Draft{amount_staked:u64}
//                                          | Pending | Passed | Failed. Tag byte +
//                                          (8 bytes ONLY for Draft). All fields
//                                          AFTER `state` are at a variable offset.
//   base_vault: Pubkey
//   quote_vault: Pubkey
//   dao: Pubkey
//   pda_bump: u8
//   question: Pubkey
//   duration_in_seconds: u32
//   squads_proposal: Pubkey
//   pass_base_mint / pass_quote_mint / fail_base_mint / fail_quote_mint: Pubkey
//
// The leading fixed region (number/proposer/timestamp_enqueued + the state TAG)
// is reliable; `state` tag byte @52 (0=Draft,1=Pending,2=Passed,3=Failed) tells
// you the verdict. Fields after `state` need Borsh decoding (Draft adds 8 bytes).

/// `Proposal.number: u32` — byte offset.
pub const PROPOSAL_NUMBER_OFFSET: usize = 8;
/// `Proposal.proposer: Pubkey` — byte offset.
pub const PROPOSAL_PROPOSER_OFFSET: usize = 12;
/// `Proposal.timestamp_enqueued: i64` — byte offset.
pub const PROPOSAL_TS_ENQUEUED_OFFSET: usize = 44;
/// `Proposal.state` enum tag — byte offset (0=Draft,1=Pending,2=Passed,3=Failed).
pub const PROPOSAL_STATE_TAG_OFFSET: usize = 52;

// ## Meteora DAMM v2 `Pool` (cp-amm, `#[account(zero_copy)] #[repr(C)]`,
// MeteoraAg/damm-v2 programs/cp-amm/src/state/pool.rs). Field ORDER (8-byte disc
// first; zero-copy means C layout with explicit padding fields, NOT borsh):
//
//   disc[8]
//   pool_fees: PoolFeesStruct   (base_fee, protocol/referral fee %, dynamic_fee,
//                                init_sqrt_price; nested zero-copy structs)
//   token_a_mint: Pubkey
//   token_b_mint: Pubkey
//   token_a_vault: Pubkey
//   token_b_vault: Pubkey
//   whitelisted_vault: Pubkey
//   padding_0: [u8; 32]
//   liquidity: u128
//   padding_1: u128
//   protocol_a_fee: u64
//   protocol_b_fee: u64
//   padding_2: u128
//   sqrt_min_price: u128
//   sqrt_max_price: u128
//   sqrt_price: u128            <-- the load-bearing INSTANTANEOUS price (Q64.64)
//   activation_point: u64
//   activation_type: u8         (0 = by slot, 1 = by timestamp)
//   pool_status / token_a_flag / token_b_flag / collect_fee_mode / pool_type /
//   fee_version / padding_3: u8 …
//   fee_a_per_liquidity / fee_b_per_liquidity: [u8;32]  (cumulative FEE, U256)
//   permanent_lock_liquidity: u128
//   metrics: PoolMetrics
//   creator: Pubkey
//   token_a_amount / token_b_amount: u64
//   layout_version: u8 + padding …
//   reward_infos: [RewardInfo; NUM_REWARDS]
//
// IMPORTANT (F5): there is NO TWAP / cumulative-price observation in cp-amm —
// `sqrt_price` is the spot price at last touch, and `fee_*_per_liquidity` are FEE
// accumulators, not price. Use [`futarchy_spot_twap`] for the manipulation-
// resistant TWAP. The exact byte offset of `sqrt_price` is NOT hand-pinned here:
// computing it requires the full C-layout/padding of the nested zero-copy
// `PoolFeesStruct`/`BaseFeeStruct`/`DynamicFeeStruct`, which is error-prone by
// hand. F5 (if it ends up reading a Meteora pool at all) MUST pin `sqrt_price`'s
// offset against a LIVE pool account dump and/or the published cp-amm IDL before
// relying on it. Field ORDER above is from source and is authoritative; the
// numeric offset is the deferred unknown.

// ─────────────────────────────────────────────────────────────────────────────
// Little-endian field readers (out-of-bounds -> InvalidAccount)
// ─────────────────────────────────────────────────────────────────────────────

/// Read a 32-byte pubkey out of `data` at byte `off`.
pub fn read_pubkey(data: &[u8], off: usize) -> Result<Pubkey, ProgramError> {
    data.get(off..off + 32)
        .and_then(|s| s.try_into().ok())
        .ok_or_else(|| KassandraError::InvalidAccount.into())
}

/// Read a little-endian `u32` out of `data` at byte `off`.
pub fn read_u32(data: &[u8], off: usize) -> Result<u32, ProgramError> {
    data.get(off..off + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| KassandraError::InvalidAccount.into())
}

/// Read a little-endian `i64` out of `data` at byte `off`.
pub fn read_i64(data: &[u8], off: usize) -> Result<i64, ProgramError> {
    data.get(off..off + 8)
        .and_then(|s| s.try_into().ok())
        .map(i64::from_le_bytes)
        .ok_or_else(|| KassandraError::InvalidAccount.into())
}

/// Read a little-endian `u128` out of `data` at byte `off`.
pub fn read_u128(data: &[u8], off: usize) -> Result<u128, ProgramError> {
    data.get(off..off + 16)
        .and_then(|s| s.try_into().ok())
        .map(u128::from_le_bytes)
        .ok_or_else(|| KassandraError::InvalidAccount.into())
}

/// Compute the futarchy spot-market TWAP from a raw `Dao` account's bytes,
/// mirroring `Pool::get_twap()` in the v0.6 futarchy source:
///
/// ```text
/// twap = aggregator / (last_updated_timestamp - (created_at_timestamp + start_delay_seconds))
/// ```
///
/// The result is a price scaled by `1e12` (quote units per base unit). This is
/// the F5 `kass_price` primitive: it reads the spot `Pool.oracle` embedded in the
/// `Dao` (fixed offsets, variant-independent — see the layout block above).
/// Returns [`KassandraError::InvalidAccount`] if the buffer is too short, or if
/// the elapsed window is non-positive or the aggregator is zero (i.e. the TWAP
/// has not started / is not yet observable).
pub fn futarchy_spot_twap(dao_data: &[u8]) -> Result<u128, ProgramError> {
    let aggregator = read_u128(dao_data, DAO_SPOT_AGGREGATOR_OFFSET)?;
    let last_updated = read_i64(dao_data, DAO_SPOT_LAST_UPDATED_TS_OFFSET)?;
    let created_at = read_i64(dao_data, DAO_SPOT_CREATED_AT_TS_OFFSET)?;
    let start_delay = read_u32(dao_data, DAO_SPOT_START_DELAY_SECONDS_OFFSET)? as i64;

    let start = created_at
        .checked_add(start_delay)
        .ok_or(KassandraError::InvalidAccount)?;
    let seconds_passed = last_updated
        .checked_sub(start)
        .filter(|&d| d > 0)
        .ok_or(KassandraError::InvalidAccount)?;
    if aggregator == 0 {
        return Err(KassandraError::InvalidAccount.into());
    }
    Ok(aggregator / seconds_passed as u128)
}
