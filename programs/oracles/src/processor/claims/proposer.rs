//! `claim_proposer`: the per-proposer bond return + (Resolved) cohort reward.

use pinocchio::{
    account::AccountView as AccountInfo, address::Address as Pubkey, error::ProgramError,
    ProgramResult,
};

use crate::{
    error::KassandraError,
    processor::guards::{
        assert_key, assert_token_account, load_oracle, verify_oracle_pda,
    },
    reward,
    state::{Proposer, CLAIM_OPTION_NONE},
};

use super::common::{is_resolved, payout_and_close, PAYLOAD_LEN};

pub fn claim_proposer(
    program_id: &Pubkey,
    accounts: &mut [AccountInfo],
    payload: &[u8],
) -> ProgramResult {
    if payload.len() != PAYLOAD_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let nonce = u64::from_le_bytes(payload[0..8].try_into().unwrap());

    let [oracle_ai, proposer_ai, dest_kass_ai, stake_vault_ai, rent_recipient_ai, token_prog_ai, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    assert_key(token_prog_ai, &pinocchio_token::ID)?;

    let oracle = load_oracle(oracle_ai, program_id)?;
    let resolved = is_resolved(&oracle)?;
    verify_oracle_pda(program_id, oracle_ai, &oracle, nonce)?;
    assert_key(stake_vault_ai, &oracle.stake_vault)?;

    // Load + bind the proposer (type guard + this-oracle membership).
    let proposer = load_proposer_checked(proposer_ai, program_id, oracle_ai.address())?;
    assert_token_account(dest_kass_ai, &oracle.kass_mint, &proposer.authority)?;
    assert_key(rent_recipient_ai, &proposer.authority)?;

    // Base return per proposer:
    //  * DISQUALIFIED → 0: a disqualified proposer FORFEITS the whole bond. It has
    //    been fully distributed already — into `bond_pool` (`slashed_amount`) AND,
    //    on a CHALLENGE disqualify, a `kass_fee = bond − slashed_amount` was paid
    //    out of `stake_vault` to the challenger by `settle_challenge`. So
    //    `bond − slashed_amount` would over-pay the fraudster exactly that
    //    already-gone `kass_fee` → vault shortfall for the last claimant. Forfeit
    //    everything. (No-show / no-facts dead-end set `slashed_amount == bond`, so
    //    this is a no-op there; it corrects only the challenge-disqualify row.)
    //  * SURVIVOR → `bond − slashed_amount`. Any survivor slash (flip) already
    //    funded `bond_pool`, so deducting it prevents the flip-survivor double-pay
    //    (full bond returned AND the slash paid out as rewards). Honest survivor →
    //    `slashed_amount == 0` → full `bond`; flip-slashed survivor → `bond − flip`
    //    (applies on BOTH terminal phases — a flipped proposer is NOT disqualified
    //    and can survive to Resolved OR tie into InvalidDeadend).
    // The reward (Resolved + surviving + correct only) keeps `bond` as its
    // pro-rata weight, matching S1's `total_correct_proposer_stake = Σ bond`, so
    // `Σ(bond − slashed) + reward_pool = Σbond − bond_pool + bond_pool = Σbond`.
    let base = if proposer.is_disqualified() {
        0
    } else {
        proposer.bond.saturating_sub(proposer.slashed_amount)
    };
    // "Correct" = the proposer backed the resolved option. On the disputed path
    // that is the AI `claim_option`; on the uncontested (all-agree) path proposers
    // never submitted an AI claim (`claim_option == CLAIM_OPTION_NONE`), so fall
    // back to their `original_option`. A no-show in a DISPUTED oracle also carries
    // `claim_option == NONE`, but it is always disqualified (excluded below), so
    // this fallback rewards ONLY the uncontested cohort.
    let backed_resolved = proposer.claim_option == oracle.resolved_option
        || (proposer.claim_option == CLAIM_OPTION_NONE
            && proposer.original_option == oracle.resolved_option);
    let reward = if resolved && !proposer.is_disqualified() && backed_resolved {
        let (proposer_bucket, _) = reward::reward_buckets(
            oracle.reward_pool,
            oracle.reward_proposer_weight,
            oracle.reward_fact_weight,
            oracle.total_correct_proposer_stake,
            oracle.total_approved_fact_stake,
        );
        reward::proposer_reward(
            proposer.bond,
            proposer_bucket,
            oracle.total_correct_proposer_stake,
        )
    } else {
        0
    };
    let amount = base
        .checked_add(reward)
        .ok_or(ProgramError::ArithmeticOverflow)?;

    payout_and_close(
        oracle_ai,
        stake_vault_ai,
        dest_kass_ai,
        proposer_ai,
        rent_recipient_ai,
        nonce,
        oracle.bump,
        amount,
    )
}

/// Load + type-check a [`Proposer`] and require it belongs to `oracle`.
fn load_proposer_checked(
    account: &AccountInfo,
    program_id: &Pubkey,
    oracle: &Pubkey,
) -> Result<Proposer, ProgramError> {
    let proposer = crate::processor::guards::load_proposer(account, program_id)?;
    if &proposer.oracle != oracle {
        return Err(KassandraError::InvalidAccount.into());
    }
    Ok(proposer)
}
