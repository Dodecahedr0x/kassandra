//! The `settle_challenge` instruction entry point.

use pinocchio::{
    account::AccountView as AccountInfo, address::Address as Pubkey, cpi::Signer,
    error::ProgramError, instruction::InstructionAccount, ProgramResult,
};
use pinocchio_token::instructions::Transfer;

use crate::{
    clock::{now, require_phase},
    cpi::metadao,
    error::KassandraError,
    processor::guards::{
        assert_key, assert_owned_by_program, assert_token_account, load_ai_claim, load_oracle,
        load_proposer, verify_oracle_pda,
    },
    state::{Market, Oracle, Phase},
};

use super::twap::{fee_amount, verify_and_read_twap, PAYLOAD_LEN};

pub fn process(program_id: &Pubkey, accounts: &mut [AccountInfo], payload: &[u8]) -> ProgramResult {
    if payload.len() != PAYLOAD_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let oracle_nonce = u64::from_le_bytes(payload[0..8].try_into().unwrap());

    let [oracle_ai, market_ai, ai_claim_ai, proposer_ai, question_ai, pass_amm_ai, fail_amm_ai, cv_prog_ai, cv_event_auth_ai, token_prog_ai, stake_vault_ai, kass_vault_ai, kass_vault_underlying_ai, pass_kass_mint_ai, fail_kass_mint_ai, oracle_pass_kass_ai, oracle_fail_kass_ai, escrow_vault_ai, proposer_usdc_ai, challenger_usdc_dest_ai, challenger_kass_ai, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    assert_key(cv_prog_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    assert_key(token_prog_ai, &pinocchio_token::ID)?;

    // --- oracle + phase gate -----------------------------------------------
    let mut oracle: Oracle = load_oracle(oracle_ai, program_id)?;
    require_phase(&oracle, Phase::Challenge)?;

    // The oracle PDA is the question's resolver, signed below with
    // `[b"oracle", nonce_le, [bump]]`.
    verify_oracle_pda(program_id, oracle_ai, &oracle, oracle_nonce)?;

    // --- market load + binding ---------------------------------------------
    assert_owned_by_program(market_ai, program_id)?;
    if market_ai.data_len() < Market::LEN {
        return Err(KassandraError::InvalidAccount.into());
    }
    let mut market: Market = {
        let data = market_ai.try_borrow()?;
        bytemuck::pod_read_unaligned::<Market>(&data[..Market::LEN])
    };
    if market.account_type != crate::state::AccountType::Market.as_u8() {
        return Err(KassandraError::InvalidAccount.into());
    }
    if market.oracle != *oracle_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }
    if market.is_settled() {
        return Err(KassandraError::AlreadySettled.into());
    }

    // --- TWAP window gate ---------------------------------------------------
    let now = now()?;
    if now < market.twap_end {
        return Err(KassandraError::TwapWindowOpen.into());
    }

    // --- bind the recorded accounts -----------------------------------------
    assert_key(ai_claim_ai, &market.ai_claim)?;
    assert_key(proposer_ai, &market.proposer)?;
    assert_key(question_ai, &market.question)?;
    assert_key(pass_amm_ai, &market.pass_amm)?;
    assert_key(fail_amm_ai, &market.fail_amm)?;
    // A challenger must not be able to alias the two pools.
    if pass_amm_ai.address() == fail_amm_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }

    // --- HARD AMM binding: each AMM ↔ this market's conditional mint pair ----
    let (pass_kass_mint, _) = metadao::conditional_token_mint_pda(&market.kass_vault, 0);
    let (fail_kass_mint, _) = metadao::conditional_token_mint_pda(&market.kass_vault, 1);
    let (pass_usdc_mint, _) = metadao::conditional_token_mint_pda(&market.usdc_vault, 0);
    let (fail_usdc_mint, _) = metadao::conditional_token_mint_pda(&market.usdc_vault, 1);

    let pass_twap = verify_and_read_twap(pass_amm_ai, &pass_kass_mint, &pass_usdc_mint)?;
    let fail_twap = verify_and_read_twap(fail_amm_ai, &fail_kass_mint, &fail_usdc_mint)?;

    // --- claim + proposer ---------------------------------------------------
    let ai_claim = load_ai_claim(ai_claim_ai, program_id)?;
    if ai_claim.oracle != *oracle_ai.address() || ai_claim.proposer != *proposer_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }
    let mut proposer = load_proposer(proposer_ai, program_id)?;
    if proposer.oracle != *oracle_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }

    // --- bind the physical-settlement accounts ------------------------------
    // The redeem + fee CPIs below need: the stake vault (redeem dest + KASS-fee
    // source), the KASS conditional vault + its underlying ATA, the conditional
    // KASS mints + the oracle-PDA-owned holders the bond was split into, the USDC
    // escrow, and the proposer/challenger payout accounts. Bind every one to the
    // recorded `Market`/`Oracle` so a settle cranker cannot substitute accounts.
    assert_key(stake_vault_ai, &oracle.stake_vault)?;
    assert_key(kass_vault_ai, &market.kass_vault)?;
    assert_key(pass_kass_mint_ai, &pass_kass_mint)?;
    assert_key(fail_kass_mint_ai, &fail_kass_mint)?;
    assert_key(oracle_pass_kass_ai, &market.oracle_pass_kass)?;
    assert_key(oracle_fail_kass_ai, &market.oracle_fail_kass)?;
    assert_key(escrow_vault_ai, &market.challenger_usdc_vault)?;
    // The redeem vault's underlying token account must be the one the vault
    // records (the same ATA the bond was split into at open_challenge).
    assert_owned_by_program(kass_vault_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    {
        let data = kass_vault_ai.try_borrow()?;
        let v_underlying_acct =
            metadao::read_pubkey(&data, metadao::VAULT_UNDERLYING_ACCOUNT_OFFSET)?;
        if &v_underlying_acct != kass_vault_underlying_ai.address() {
            return Err(KassandraError::InvalidAccount.into());
        }
    }
    // Payout destinations: pin mint + owner so the directional fees / escrow
    // return cannot be siphoned. Proposer USDC ↔ proposer.authority; challenger
    // USDC + KASS ↔ market.challenger.
    assert_token_account(proposer_usdc_ai, &oracle.usdc_mint, &proposer.authority)?;
    assert_token_account(
        challenger_usdc_dest_ai,
        &oracle.usdc_mint,
        &market.challenger,
    )?;
    assert_token_account(challenger_kass_ai, &oracle.kass_mint, &market.challenger)?;

    // --- slash trigger (u128): fail_twap * DEN > pass_twap * (DEN + NUM) -----
    // GUARD: `pass_twap == 0` ALWAYS survives. A zero pass TWAP means the pass
    // pool has no observation — i.e. NO counter-trading on the pass side — which
    // design §7 defines as "claim survives". Without this guard a challenger
    // could crank ONLY the fail pool (leaving pass un-cranked at 0) and cheaply
    // flip `fail_twap*DEN > 0` true to disqualify an honest proposer. So a
    // disqualification requires a real, non-zero pass price to beat.
    // Margin params are snapshotted on the oracle (== MARKET_THRESHOLD_* by
    // default); stored as u64, widened back to u128 for the overflow-safe math.
    let market_threshold_num = oracle.market_threshold_num as u128;
    let market_threshold_den = oracle.market_threshold_den as u128;
    let disqualify = if pass_twap == 0 {
        false
    } else {
        let lhs = fail_twap
            .checked_mul(market_threshold_den)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let rhs = pass_twap
            .checked_mul(market_threshold_den + market_threshold_num)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        lhs > rhs
    };

    // PASS-side [1,0] survives, FAIL-side [0,1] disqualifies.
    let numerators: [u32; 2] = if disqualify { [0, 1] } else { [1, 0] };

    // Directional KASS fee on a SUCCESSFUL challenge (disqualify): a carve-out of
    // the bond to the challenger. The proposer's `bond_pool` contribution becomes
    // `bond − kass_fee` (NOT the full bond), keeping the per-proposer identity
    // `slashed_amount == bond_pool contribution`.
    //
    // DEFENSIVE CAP (belt-and-suspenders): cap the fee to the proposer's REMAINING
    // un-slashed bond (`bond − slashed_amount`). A proposer flip-slashed earlier in
    // finalize_ai_claims already contributed `slashed_amount` to bond_pool; the
    // carve-out tops that up to `bond − kass_fee`, which must stay ≥ the prior
    // slash. The `set_config` joint bound (`flip_slash_frac + success_kass_fee_frac
    // ≤ 1`) guarantees that for valid configs (cap is then a no-op), but capping
    // here means even a hypothetically-bad config can never underflow the carve-out
    // / brick settlement, nor transfer more KASS than is left in stake_vault. The
    // capped value drives BOTH the accounting and the KASS transfer below.
    let remaining_bond = proposer.bond.saturating_sub(proposer.slashed_amount);
    let kass_fee = fee_amount(
        proposer.bond,
        oracle.challenge_success_kass_fee_num,
        oracle.challenge_success_kass_fee_den,
    )?
    .min(remaining_bond);

    if disqualify && !proposer.is_disqualified() {
        // Net slash = bond − kass_fee (≥ slashed_amount by the cap). Top up any
        // prior (flip) slash to exactly that net (never double-counting, never
        // exceeding the escrowed bond): the kass_fee leaves to the challenger
        // below, the rest is the bond_pool contribution.
        let net_slash = proposer
            .bond
            .checked_sub(kass_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let delta = net_slash.saturating_sub(proposer.slashed_amount);
        proposer.disqualified = 1;
        proposer.slashed = 1;
        proposer.slashed_amount = net_slash;
        oracle.bond_pool = oracle
            .bond_pool
            .checked_add(delta)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        oracle.surviving_count = oracle
            .surviving_count
            .checked_sub(1)
            .ok_or(ProgramError::ArithmeticOverflow)?;
    }

    // --- program-signed resolve_question (oracle PDA is the resolver) -------
    let (cv_event_auth, _) = metadao::event_authority_pda(&metadao::CONDITIONAL_VAULT_ID);
    assert_key(cv_event_auth_ai, &cv_event_auth)?;

    let resolve_data = metadao::resolve_question_data_binary(numerators);
    let resolve_metas = [
        InstructionAccount::writable(question_ai.address()),
        InstructionAccount::readonly_signer(oracle_ai.address()),
        InstructionAccount::readonly(cv_event_auth_ai.address()),
        InstructionAccount::readonly(cv_prog_ai.address()),
    ];
    let resolve_infos = [&*question_ai, &*oracle_ai, &*cv_event_auth_ai, &*cv_prog_ai];
    let nonce_le = oracle_nonce.to_le_bytes();
    let bump_seed = [oracle.bump];
    let oracle_seeds = Oracle::signer_seeds(&nonce_le, &bump_seed);
    metadao::invoke_conditional_vault_signed(
        &resolve_data,
        &resolve_metas,
        &resolve_infos,
        &[Signer::from(&oracle_seeds)],
    )?;

    // --- physical redeem: bond's conditional KASS → stake_vault -------------
    // redeem_tokens (InteractWithVault, same shape as the open_challenge split):
    //   0 question  1 kass_vault(w)  2 kass_vault_underlying(w)
    //   3 authority=oracle PDA(signer)  4 stake_vault(w, user_underlying)
    //   5 token_program  6 cv_event_auth  7 cv_program
    //   remaining: pass_mint(w) fail_mint(w) oracle_pass_kass(w) oracle_fail_kass(w)
    // The question is now resolved, so the winning side redeems 1:1 and the losing
    // side → 0 — the FULL `bond` KASS the proposer split lands in `stake_vault`.
    let redeem_data = metadao::redeem_tokens_data();
    let redeem_metas = [
        InstructionAccount::readonly(question_ai.address()),
        InstructionAccount::writable(kass_vault_ai.address()),
        InstructionAccount::writable(kass_vault_underlying_ai.address()),
        InstructionAccount::readonly_signer(oracle_ai.address()), // authority (oracle PDA)
        InstructionAccount::writable(stake_vault_ai.address()),   // user_underlying (dest)
        InstructionAccount::readonly(token_prog_ai.address()),
        InstructionAccount::readonly(cv_event_auth_ai.address()),
        InstructionAccount::readonly(cv_prog_ai.address()),
        InstructionAccount::writable(pass_kass_mint_ai.address()),
        InstructionAccount::writable(fail_kass_mint_ai.address()),
        InstructionAccount::writable(oracle_pass_kass_ai.address()),
        InstructionAccount::writable(oracle_fail_kass_ai.address()),
    ];
    let redeem_infos = [
        &*question_ai,
        &*kass_vault_ai,
        &*kass_vault_underlying_ai,
        &*oracle_ai,
        &*stake_vault_ai,
        &*token_prog_ai,
        &*cv_event_auth_ai,
        &*cv_prog_ai,
        &*pass_kass_mint_ai,
        &*fail_kass_mint_ai,
        &*oracle_pass_kass_ai,
        &*oracle_fail_kass_ai,
    ];
    metadao::invoke_conditional_vault_signed(
        &redeem_data,
        &redeem_metas,
        &redeem_infos,
        &[Signer::from(&oracle_seeds)],
    )?;

    // --- directional fee routing (oracle PDA signs every move) --------------
    let challenger_usdc = market.challenger_usdc;
    if disqualify {
        // Successful challenge: KASS fee carved out of the (now-redeemed) bond in
        // stake_vault → challenger; full USDC escrow returned to the challenger.
        if kass_fee > 0 {
            Transfer::new(stake_vault_ai, challenger_kass_ai, oracle_ai, kass_fee)
                .invoke_signed(&[Signer::from(&oracle_seeds)])?;
        }
        if challenger_usdc > 0 {
            Transfer::new(
                escrow_vault_ai,
                challenger_usdc_dest_ai,
                oracle_ai,
                challenger_usdc,
            )
            .invoke_signed(&[Signer::from(&oracle_seeds)])?;
        }
    } else {
        // Failed challenge: bond stays the proposer's (redeemed into stake_vault).
        // USDC fee → proposer; the remainder of the escrow → challenger. The split
        // is exact: usdc_fee + return == challenger_usdc.
        let usdc_fee = fee_amount(
            challenger_usdc,
            oracle.challenge_fail_usdc_fee_num,
            oracle.challenge_fail_usdc_fee_den,
        )?;
        let challenger_return = challenger_usdc
            .checked_sub(usdc_fee)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        if usdc_fee > 0 {
            Transfer::new(escrow_vault_ai, proposer_usdc_ai, oracle_ai, usdc_fee)
                .invoke_signed(&[Signer::from(&oracle_seeds)])?;
        }
        if challenger_return > 0 {
            Transfer::new(
                escrow_vault_ai,
                challenger_usdc_dest_ai,
                oracle_ai,
                challenger_return,
            )
            .invoke_signed(&[Signer::from(&oracle_seeds)])?;
        }
    }

    // --- persist (oracle, proposer, market) ---------------------------------
    market.settled = 1;
    // One fewer OPEN challenge market. Task 12 gates final plurality recompute
    // on this reaching 0, so every challenged proposer is settled first.
    oracle.open_challenge_count = oracle
        .open_challenge_count
        .checked_sub(1)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    {
        let mut data = market_ai.try_borrow_mut()?;
        data[..Market::LEN].copy_from_slice(bytemuck::bytes_of(&market));
    }
    {
        let mut data = proposer_ai.try_borrow_mut()?;
        data[..crate::state::Proposer::LEN].copy_from_slice(bytemuck::bytes_of(&proposer));
    }
    {
        let mut data = oracle_ai.try_borrow_mut()?;
        data[..Oracle::LEN].copy_from_slice(bytemuck::bytes_of(&oracle));
    }

    Ok(())
}
