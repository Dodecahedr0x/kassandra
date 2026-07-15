//! `claim_fact`: the fact SUBMITTER's stake return + (Resolved) fact reward.

use pinocchio::{
    account::AccountView as AccountInfo, address::Address as Pubkey, error::ProgramError,
    ProgramResult,
};

use crate::{
    error::KassandraError,
    processor::guards::{
        assert_key, assert_token_account, load_fact, load_oracle, verify_oracle_pda,
    },
    reward,
    state::{Fact, Oracle},
};

use super::common::{is_resolved, payout_and_close, PAYLOAD_LEN};

pub fn claim_fact(
    program_id: &Pubkey,
    accounts: &mut [AccountInfo],
    payload: &[u8],
) -> ProgramResult {
    if payload.len() != PAYLOAD_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let nonce = u64::from_le_bytes(payload[0..8].try_into().unwrap());

    let [oracle_ai, fact_ai, dest_kass_ai, stake_vault_ai, rent_recipient_ai, token_prog_ai, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    assert_key(token_prog_ai, &pinocchio_token::ID)?;

    let oracle = load_oracle(oracle_ai, program_id)?;
    let resolved = is_resolved(&oracle)?;
    verify_oracle_pda(program_id, oracle_ai, &oracle, nonce)?;
    assert_key(stake_vault_ai, &oracle.stake_vault)?;

    let fact = load_fact(fact_ai, program_id)?;
    if &fact.oracle != oracle_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }
    // The fact's submitter authority is `fact.proposer`.
    assert_token_account(dest_kass_ai, &oracle.kass_mint, &fact.proposer)?;
    assert_key(rent_recipient_ai, &fact.proposer)?;

    // The submitter claim CLOSES the Fact, but every `claim_fact_vote` must read
    // the Fact's disposition first. So the submitter must claim LAST: refuse to
    // close while any voter stake is still unclaimed (each `claim_fact_vote`
    // decrements these running totals as a voter claims).
    if fact.approve_stake != 0 || fact.duplicate_stake != 0 {
        return Err(KassandraError::VotersOutstanding.into());
    }

    let amount = fact_submitter_entitlement(&oracle, &fact, resolved)?;

    payout_and_close(
        oracle_ai,
        stake_vault_ai,
        dest_kass_ai,
        fact_ai,
        rent_recipient_ai,
        nonce,
        oracle.bump,
        amount,
    )
}

/// Entitlement for a fact SUBMITTER (see the module matrix). The fact's
/// disposition (agreed / duplicate / rejected) is applied on BOTH terminal
/// phases; only the reward (Resolved only) differs. On `InvalidDeadend` the
/// reward is 0 (reward_pool == 0) AND, crucially, a REJECTED submitter forfeits
/// (returns 0) — its stake funded `bond_pool`, which the InvalidDeadend finalize
/// site BURNED out of the vault, so returning it would short the vault.
fn fact_submitter_entitlement(
    oracle: &Oracle,
    fact: &Fact,
    resolved: bool,
) -> Result<u64, ProgramError> {
    if fact.is_agreed() {
        let r = if resolved {
            let (_, fact_bucket) = reward::reward_buckets(
                oracle.reward_pool,
                oracle.reward_proposer_weight,
                oracle.reward_fact_weight,
                oracle.total_correct_proposer_stake,
                oracle.total_approved_fact_stake,
            );
            reward::fact_reward(fact.stake, fact_bucket, oracle.total_approved_fact_stake)
        } else {
            0 // InvalidDeadend: no reward distribution.
        };
        return fact
            .stake
            .checked_add(r)
            .ok_or(ProgramError::ArithmeticOverflow);
    }
    if fact.is_duplicate() {
        return Ok(fact.stake); // Duplicate-dominant: stake returned, no reward/slash.
    }
    Ok(0) // Rejected submitter: 100% forfeit on both phases (still close + reclaim rent).
}
