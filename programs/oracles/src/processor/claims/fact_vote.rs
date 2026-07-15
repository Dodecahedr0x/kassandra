//! `claim_fact_vote`: the per-voter stake return / slash + (Resolved) fact reward.

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
    state::{AccountType, Fact, FactVote, VOTE_DUPLICATE},
};

use super::common::{is_resolved, payout_and_close, PAYLOAD_LEN};

/// Per-voter rejected-fact slash: `ceil(value · num / den)` in u128. `den == 0`
/// (defended; the snapshot keeps it positive) yields 0 so the entitlement
/// degrades to the full stake rather than dividing by zero.
///
/// # Why CEIL (conservation, not just rounding)
/// `finalize_facts` credits `bond_pool` with the AGGREGATE
/// `floor(Σ approve_stake · num/den)` for the rejected fact, and that whole
/// credit is later paid out as rewards. If each voter were slashed
/// `floor(stakeᵢ · num/den)`, then `Σ floor(stakeᵢ·r) ≤ floor(Σ stakeᵢ · r)` —
/// the vault could physically retain LESS than the bond_pool credit, shorting
/// the last reward claimant. Slashing each voter `ceil(stakeᵢ·r)` instead gives
/// `Σ ceil(stakeᵢ·r) ≥ (Σ stakeᵢ)·r ≥ floor(Σ·r)`, so the vault is never short;
/// any excess is conservation-safe sub-unit dust. `ceil = (v·num + den − 1)/den`.
fn slash_amount(value: u64, num: u64, den: u64) -> u64 {
    if den == 0 {
        return 0;
    }
    let scaled = (value as u128) * (num as u128) + (den as u128 - 1);
    (scaled / den as u128) as u64
}

pub fn claim_fact_vote(
    program_id: &Pubkey,
    accounts: &mut [AccountInfo],
    payload: &[u8],
) -> ProgramResult {
    if payload.len() != PAYLOAD_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let nonce = u64::from_le_bytes(payload[0..8].try_into().unwrap());

    let [oracle_ai, vote_ai, fact_ai, dest_kass_ai, stake_vault_ai, rent_recipient_ai, token_prog_ai, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    assert_key(token_prog_ai, &pinocchio_token::ID)?;

    let oracle = load_oracle(oracle_ai, program_id)?;
    let resolved = is_resolved(&oracle)?;
    verify_oracle_pda(program_id, oracle_ai, &oracle, nonce)?;
    assert_key(stake_vault_ai, &oracle.stake_vault)?;

    // FactVote carries no oracle field; bind it through the fact:
    // vote.fact == fact_ai and fact.oracle == oracle.
    let vote = load_fact_vote(vote_ai, program_id)?;
    let mut fact = load_fact(fact_ai, program_id)?;
    if &vote.fact != fact_ai.address() || &fact.oracle != oracle_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }
    assert_token_account(dest_kass_ai, &oracle.kass_mint, &vote.voter)?;
    assert_key(rent_recipient_ai, &vote.voter)?;

    // Disposition-based on BOTH terminal phases; only the reward (Resolved only)
    // differs. On InvalidDeadend reward_pool == 0 (reward 0) AND the rejected-fact
    // approve-voter is STILL slashed: its slashed fraction funded `bond_pool`,
    // which the InvalidDeadend finalize site BURNED out of the vault, so returning
    // the full stake would short the vault.
    let amount = if vote.kind == VOTE_DUPLICATE {
        // Duplicate-voter: never slashed or rewarded, on any fact / either phase.
        vote.stake
    } else if fact.is_agreed() {
        // Approve-voter on an agreed fact: stake + pro-rata fact reward (Resolved
        // only; 0 on InvalidDeadend since reward_pool == 0).
        let r = if resolved {
            let (_, fact_bucket) = reward::reward_buckets(
                oracle.reward_pool,
                oracle.reward_proposer_weight,
                oracle.reward_fact_weight,
                oracle.total_correct_proposer_stake,
                oracle.total_approved_fact_stake,
            );
            reward::fact_reward(vote.stake, fact_bucket, oracle.total_approved_fact_stake)
        } else {
            0
        };
        vote.stake
            .checked_add(r)
            .ok_or(ProgramError::ArithmeticOverflow)?
    } else if fact.is_duplicate() {
        // Approve-voter on a duplicate-dominant fact: stake, no reward, no slash.
        vote.stake
    } else {
        // Approve-voter on a rejected fact: the slashed fraction already funded
        // bond_pool (burned on a dead-end); reclaim only the remainder.
        let slash = slash_amount(
            vote.stake,
            oracle.fact_vote_slash_num,
            oracle.fact_vote_slash_den,
        );
        vote.stake.saturating_sub(slash)
    };

    // Decrement the fact's running voter-stake total so the submitter's
    // `claim_fact` can tell when every voter has claimed (and only THEN close
    // the Fact). This keeps the Fact alive for all voters' disposition reads.
    // `saturating_sub` defends against any stray double-count; in the normal
    // flow `approve_stake`/`duplicate_stake` is exactly Σ voter stakes.
    if vote.kind == VOTE_DUPLICATE {
        fact.duplicate_stake = fact.duplicate_stake.saturating_sub(vote.stake);
    } else {
        fact.approve_stake = fact.approve_stake.saturating_sub(vote.stake);
    }
    {
        let mut data = fact_ai.try_borrow_mut()?;
        data[..Fact::LEN].copy_from_slice(bytemuck::bytes_of(&fact));
    }

    payout_and_close(
        oracle_ai,
        stake_vault_ai,
        dest_kass_ai,
        vote_ai,
        rent_recipient_ai,
        nonce,
        oracle.bump,
        amount,
    )
}

/// Load + type-check a [`FactVote`] (owner == program, size, tag).
fn load_fact_vote(account: &AccountInfo, program_id: &Pubkey) -> Result<FactVote, ProgramError> {
    crate::processor::guards::assert_owned_by_program(account, program_id)?;
    if account.data_len() < FactVote::LEN {
        return Err(KassandraError::InvalidAccount.into());
    }
    let vote: FactVote = {
        let data = account.try_borrow()?;
        bytemuck::pod_read_unaligned::<FactVote>(&data[..FactVote::LEN])
    };
    if vote.account_type != AccountType::FactVote.as_u8() {
        return Err(KassandraError::InvalidAccount.into());
    }
    Ok(vote)
}
