//! Shared claim primitives: the payload length, the terminal-phase gate, and the
//! program-signed payout+close.

use pinocchio::{
    account::AccountView as AccountInfo, cpi::Signer, error::ProgramError, ProgramResult,
};
use pinocchio_token::instructions::Transfer;

use crate::{
    processor::guards::{drain_lamports, require_terminal},
    state::{Oracle, Phase},
};

/// Exact payload length: `oracle_nonce[8]`.
pub(super) const PAYLOAD_LEN: usize = 8;

/// Require a terminal oracle ([`require_terminal`]) and report whether it is
/// [`Phase::Resolved`] (so the caller knows whether rewards apply).
///
/// # `resolve_deadend` (F4) oracles — no special-casing
/// An oracle force-resolved from `InvalidDeadend` → `Resolved` by the DAO
/// (`resolve_deadend`, F4) carries `reward_pool == 0` and zero cohort totals
/// (`finalize_oracle` only stamps those on the organic Resolved branch; F4 just
/// flips the phase + sets `resolved_option`). So `resolved == true` here but
/// every reward term is 0 → claims pay **non-slashed principal only**, no
/// rewards: IDENTICAL economics to the plain `InvalidDeadend` branch. This is
/// exactly the dead-end settlement rule (a non-outcome distributes nothing): the
/// slashed `bond_pool` + the `reward_emission` were already BURNED out of
/// `stake_vault` at the InvalidDeadend finalize site (`finalize_oracle` /
/// `finalize_no_facts`), so the vault holds only the returnable principal whether
/// or not governance later flips the phase to `Resolved`. No marker / no
/// claim-path branch on "resolved-from-dead-end" is needed — the `reward_pool ==
/// 0` stamp already makes both terminal phases pay identically.
pub(super) fn is_resolved(oracle: &Oracle) -> Result<bool, ProgramError> {
    require_terminal(oracle)?;
    Ok(oracle.phase() == Some(Phase::Resolved))
}

/// Transfer `amount` KASS from `stake_vault` → `dest`, program-signed by the
/// oracle PDA (`[b"oracle", nonce_le, [bump]]`). A zero amount is a no-op (a
/// rejected fact submitter still closes + reclaims rent). Then CLOSE `claimant`,
/// draining its rent lamports to `rent_recipient` and zeroing its data so a
/// second claim finds nothing.
#[allow(clippy::too_many_arguments)]
pub(super) fn payout_and_close(
    oracle_ai: &AccountInfo,
    stake_vault: &AccountInfo,
    dest: &AccountInfo,
    claimant: &mut AccountInfo,
    rent_recipient: &mut AccountInfo,
    nonce: u64,
    bump: u8,
    amount: u64,
) -> ProgramResult {
    if amount > 0 {
        let nonce_le = nonce.to_le_bytes();
        let bump_seed = [bump];
        let seeds = Oracle::signer_seeds(&nonce_le, &bump_seed);
        Transfer::new(stake_vault, dest, oracle_ai, amount)
            .invoke_signed(&[Signer::from(&seeds)])?;
    }

    // Drain rent lamports to the recipient, then zero the account (data len /
    // lamports / owner). Done in this order so the instruction stays balanced.
    drain_lamports(claimant, rent_recipient)?;
    claimant.close()
}
