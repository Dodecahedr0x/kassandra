//! Directional-fee arithmetic and the AMM-bound slot-weighted TWAP read.

use pinocchio::{
    account::AccountView as AccountInfo, address::Address as Pubkey, error::ProgramError,
};

use crate::{cpi::metadao, error::KassandraError};

/// Exact payload length: `oracle_nonce[8]`.
pub(super) const PAYLOAD_LEN: usize = 8;

/// `value × num / den` in u128, checked back into `u64`. `den == 0` (a malformed
/// fee config) is rejected as [`KassandraError::InvalidConfig`]. Used for both
/// directional fees (KASS fee on a successful challenge, USDC fee on a failed
/// one).
pub(super) fn fee_amount(value: u64, num: u64, den: u64) -> Result<u64, ProgramError> {
    if den == 0 {
        return Err(KassandraError::InvalidConfig.into());
    }
    let scaled = (value as u128)
        .checked_mul(num as u128)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    u64::try_from(scaled / den as u128).map_err(|_| ProgramError::ArithmeticOverflow)
}

/// Verify `amm` is owned by `AMM_ID`, carries the `Amm` Anchor discriminator,
/// and is bound to `(expected_base, expected_quote)`, then return its
/// slot-weighted TWAP (`aggregator / slots_passed`, or `0` if the market never
/// produced an observation). This is the hard binding the prompt requires: the
/// AMM must be THIS market's pass/fail conditional pool.
pub(super) fn verify_and_read_twap(
    amm: &AccountInfo,
    expected_base: &Pubkey,
    expected_quote: &Pubkey,
) -> Result<u128, ProgramError> {
    // Bind the AMM to this market's conditional (base, quote) pair (owner +
    // length + `Amm` discriminator + exact mint pair). Shared with
    // `open_challenge`, which now enforces the SAME binding at open so an
    // unbindable AMM can never be recorded (see `metadao::assert_amm_bound`).
    metadao::assert_amm_bound(amm, expected_base, expected_quote)?;
    let data = amm.try_borrow()?;
    let created_at = metadao::read_u64(&data, metadao::AMM_CREATED_AT_SLOT_OFFSET)?;
    let last_updated = metadao::read_u64(&data, metadao::AMM_LAST_UPDATED_SLOT_OFFSET)?;
    let aggregator = metadao::read_u128(&data, metadao::AMM_AGGREGATOR_OFFSET)?;
    let start_delay = metadao::read_u64(&data, metadao::AMM_START_DELAY_SLOTS_OFFSET)?;

    // Mirror the v0.4.2 AMM `get_twap()`:
    //   aggregator / (last_updated - (created_at + start_delay_slots)).
    // No observations (or no elapsed slots past the start delay) => no price
    // signal => 0 (a market with no counter-trading => claim survives, §7).
    let start_slot = created_at.saturating_add(start_delay);
    let slots = last_updated.saturating_sub(start_slot);
    if slots == 0 || aggregator == 0 {
        return Ok(0);
    }
    Ok(aggregator / slots as u128)
}
