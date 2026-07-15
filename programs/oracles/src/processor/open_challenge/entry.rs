//! The `open_challenge` instruction entry point.

use bytemuck::Zeroable;
use pinocchio::{
    account::AccountView as AccountInfo,
    address::Address as Pubkey,
    cpi::{Seed, Signer},
    error::ProgramError,
    instruction::InstructionAccount,
    ProgramResult,
};
use pinocchio_token::instructions::{InitializeAccount3, Transfer};
use pinocchio_token::state::Account as TokenAccount;

use crate::{
    clock::{now, require_before_end, require_phase},
    config::KASS_PRICE_SCALE,
    cpi::metadao,
    error::KassandraError,
    price::kass_price,
    processor::guards::{
        assert_key, assert_owned_by_program, assert_signer, assert_token_account, create_pda,
        load_ai_claim, load_oracle, load_proposer, load_protocol, verify_oracle_pda,
    },
    rent::minimum_rent,
    state::{AccountType, Market, Oracle, Phase},
};

/// Exact payload length: oracle_nonce[8].
const PAYLOAD_LEN: usize = 8;

/// Assert `account` is an SPL token account owned (token authority) by
/// `oracle_key` on `expected_mint`, else [`KassandraError::InvalidAccount`].
/// Defense-in-depth on the conditional-KASS split destinations: the
/// conditional_vault enforces the same constraints, but a clean local error is
/// clearer than a downstream MetaDAO custom error and pins the recorded
/// `Market.oracle_{pass,fail}_kass` contract for Task 11.
pub fn process(program_id: &Pubkey, accounts: &mut [AccountInfo], payload: &[u8]) -> ProgramResult {
    if payload.len() != PAYLOAD_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }
    let oracle_nonce = u64::from_le_bytes(payload[0..8].try_into().unwrap());

    let [oracle_ai, ai_claim_ai, proposer_ai, market_ai, challenger_ai, question_ai, kass_vault_ai, usdc_vault_ai, pass_amm_ai, fail_amm_ai, stake_vault_ai, kass_vault_underlying_ai, pass_mint_ai, fail_mint_ai, oracle_pass_kass_ai, oracle_fail_kass_ai, cv_prog_ai, token_prog_ai, system_prog_ai, cv_event_auth_ai, protocol_ai, kass_dao_ai, usdc_mint_ai, challenger_usdc_src_ai, escrow_vault_ai, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // --- signer + program ids ----------------------------------------------
    assert_signer(challenger_ai)?;
    assert_key(cv_prog_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    assert_key(token_prog_ai, &pinocchio_token::ID)?;
    assert_key(system_prog_ai, &pinocchio_system::ID)?;

    // --- oracle + phase / window gates -------------------------------------
    let mut oracle: Oracle = load_oracle(oracle_ai, program_id)?;
    require_phase(&oracle, Phase::Challenge)?;
    let now = now()?;
    require_before_end(&oracle, now)?;

    // The oracle PDA (whose seeds sign the bond split below) must match the
    // passed account.
    verify_oracle_pda(program_id, oracle_ai, &oracle, oracle_nonce)?;

    // --- claim binding ------------------------------------------------------
    let mut ai_claim = load_ai_claim(ai_claim_ai, program_id)?;
    if ai_claim.oracle != *oracle_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }
    if ai_claim.is_challenged() {
        return Err(KassandraError::AlreadyChallenged.into());
    }
    if ai_claim.proposer != *proposer_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }

    // --- proposer binding ---------------------------------------------------
    let proposer = load_proposer(proposer_ai, program_id)?;
    if proposer.oracle != *oracle_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }
    // A disqualified proposer's claim is already out — nothing to challenge.
    if proposer.is_disqualified() {
        return Err(KassandraError::Unauthorized.into());
    }

    // --- stake vault --------------------------------------------------------
    assert_key(stake_vault_ai, &oracle.stake_vault)?;

    // --- verify the MetaDAO question binds to THIS oracle -------------------
    // (Offsets are the single-source-of-truth consts in `cpi::metadao`.)
    assert_owned_by_program(question_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    {
        let data = question_ai.try_borrow()?;
        let q_oracle = metadao::read_pubkey(&data, metadao::QUESTION_ORACLE_OFFSET)?;
        if &q_oracle != oracle_ai.address() {
            return Err(KassandraError::InvalidAccount.into());
        }
        let num_outcomes = metadao::read_u32(&data, metadao::QUESTION_NUM_OUTCOMES_LEN_OFFSET)?;
        if num_outcomes != 2 {
            return Err(KassandraError::InvalidAccount.into());
        }
    }

    // --- verify the KASS conditional vault ----------------------------------
    assert_owned_by_program(kass_vault_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    {
        let data = kass_vault_ai.try_borrow()?;
        let v_question = metadao::read_pubkey(&data, metadao::VAULT_QUESTION_OFFSET)?;
        let v_underlying = metadao::read_pubkey(&data, metadao::VAULT_UNDERLYING_MINT_OFFSET)?;
        let v_underlying_acct =
            metadao::read_pubkey(&data, metadao::VAULT_UNDERLYING_ACCOUNT_OFFSET)?;
        if &v_question != question_ai.address()
            || v_underlying != oracle.kass_mint
            || &v_underlying_acct != kass_vault_underlying_ai.address()
        {
            return Err(KassandraError::InvalidAccount.into());
        }
    }

    // --- verify the USDC conditional vault ----------------------------------
    assert_owned_by_program(usdc_vault_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    {
        let data = usdc_vault_ai.try_borrow()?;
        let v_question = metadao::read_pubkey(&data, metadao::VAULT_QUESTION_OFFSET)?;
        let v_underlying = metadao::read_pubkey(&data, metadao::VAULT_UNDERLYING_MINT_OFFSET)?;
        if &v_question != question_ai.address() || v_underlying != oracle.usdc_mint {
            return Err(KassandraError::InvalidAccount.into());
        }
    }

    // --- verify the conditional KASS mints derive from the KASS vault -------
    let (expect_pass_mint, _) = metadao::conditional_token_mint_pda(kass_vault_ai.address(), 0);
    let (expect_fail_mint, _) = metadao::conditional_token_mint_pda(kass_vault_ai.address(), 1);
    assert_key(pass_mint_ai, &expect_pass_mint)?;
    assert_key(fail_mint_ai, &expect_fail_mint)?;

    // --- bind the pass/fail AMMs NOW (owner + `Amm` disc + exact conditional
    //     (KASS,USDC) mint pair per outcome), and require pass_amm != fail_amm.
    // This MUST happen at open, not only at settle: settle pins each AMM to the
    // address RECORDED here, so a market recorded with an unbindable AMM (wrong
    // mints, or the same account twice) could never settle. That would leave
    // `open_challenge_count > 0` forever, blocking `finalize_oracle` and locking
    // every stake in the oracle permanently. (Same binding `settle_challenge`
    // re-checks before reading each TWAP.)
    let (expect_pass_usdc, _) = metadao::conditional_token_mint_pda(usdc_vault_ai.address(), 0);
    let (expect_fail_usdc, _) = metadao::conditional_token_mint_pda(usdc_vault_ai.address(), 1);
    metadao::assert_amm_bound(pass_amm_ai, &expect_pass_mint, &expect_pass_usdc)?;
    metadao::assert_amm_bound(fail_amm_ai, &expect_fail_mint, &expect_fail_usdc)?;
    if pass_amm_ai.address() == fail_amm_ai.address() {
        return Err(KassandraError::InvalidAccount.into());
    }

    // --- verify the conditional-KASS split DESTINATIONS (defense-in-depth) --
    // The vault enforces these too, but a clean InvalidAccount here beats a
    // downstream MetaDAO custom error and locks the contract the docstring
    // claims: each dest is an SPL token account owned by the oracle PDA on the
    // matching conditional KASS mint. Task 11 redeems from exactly these.
    assert_token_account(oracle_pass_kass_ai, &expect_pass_mint, oracle_ai.address())?;
    assert_token_account(oracle_fail_kass_ai, &expect_fail_mint, oracle_ai.address())?;

    // --- market PDA derivation + uninit check -------------------------------
    let (expected_market, market_bump) =
        Pubkey::find_program_address(&[b"market", ai_claim_ai.address().as_ref()], program_id);
    assert_key(market_ai, &expected_market)?;
    if market_ai.lamports() != 0 || !market_ai.is_data_empty() {
        return Err(KassandraError::AlreadyChallenged.into());
    }

    // --- program-signed KASS split (oracle PDA authority) -------------------
    // Move proposer.bond KASS from oracle.stake_vault into the KASS conditional
    // vault, minting pass-KASS/fail-KASS to the oracle-PDA-owned destinations.
    // NOTE: `oracle.total_oracle_stake` is intentionally NOT decremented — the
    // KASS is still in-system, now escrowed in the conditional vault recorded on
    // the Market (Task 13 conservation counts it there).
    let (cv_event_auth, _) = metadao::event_authority_pda(&metadao::CONDITIONAL_VAULT_ID);
    assert_key(cv_event_auth_ai, &cv_event_auth)?;

    let split_data = metadao::split_tokens_data(proposer.bond);
    let split_metas = [
        InstructionAccount::readonly(question_ai.address()),
        InstructionAccount::writable(kass_vault_ai.address()),
        InstructionAccount::writable(kass_vault_underlying_ai.address()),
        InstructionAccount::readonly_signer(oracle_ai.address()), // authority (oracle PDA)
        InstructionAccount::writable(stake_vault_ai.address()),   // user_underlying
        InstructionAccount::readonly(token_prog_ai.address()),
        InstructionAccount::readonly(cv_event_auth_ai.address()),
        InstructionAccount::readonly(cv_prog_ai.address()),
        // remaining: mints then user (oracle PDA) conditional token accounts
        InstructionAccount::writable(pass_mint_ai.address()),
        InstructionAccount::writable(fail_mint_ai.address()),
        InstructionAccount::writable(oracle_pass_kass_ai.address()),
        InstructionAccount::writable(oracle_fail_kass_ai.address()),
    ];
    let split_infos = [
        &*question_ai,
        &*kass_vault_ai,
        &*kass_vault_underlying_ai,
        &*oracle_ai,
        &*stake_vault_ai,
        &*token_prog_ai,
        &*cv_event_auth_ai,
        &*cv_prog_ai,
        &*pass_mint_ai,
        &*fail_mint_ai,
        &*oracle_pass_kass_ai,
        &*oracle_fail_kass_ai,
    ];
    let nonce_le = oracle_nonce.to_le_bytes();
    let bump_seed = [oracle.bump];
    let oracle_seeds = Oracle::signer_seeds(&nonce_le, &bump_seed);
    let oracle_signer = Signer::from(&oracle_seeds);
    metadao::invoke_conditional_vault_signed(
        &split_data,
        &split_metas,
        &split_infos,
        &[oracle_signer],
    )?;

    // --- create + populate the Market PDA (challenger pays) -----------------
    let rent = minimum_rent(Market::LEN)?;
    let market_bump_seed = [market_bump];
    let market_seeds = [
        Seed::from(b"market".as_ref()),
        Seed::from(ai_claim_ai.address().as_ref()),
        Seed::from(&market_bump_seed),
    ];
    create_pda(
        challenger_ai,
        market_ai,
        &market_seeds,
        rent,
        Market::LEN,
        program_id,
    )?;

    // --- size the challenger USDC escrow via kass_price (Task C1) -----------
    // All MetaDAO market bindings are verified above; now price the escrow. The
    // escrow vault's mint must be the oracle's canonical USDC mint. `kass_price`
    // asserts `protocol` is the `[b"protocol"]` singleton (load_protocol's
    // address pin), `kass_dao == protocol.kass_dao`, and the futarchy-program
    // ownership of `kass_dao`. The returned TWAP is raw USDC per raw KASS ×
    // KASS_PRICE_SCALE, so the cross-decimal (KASS 9dp / USDC 6dp) adjustment is
    // folded in: required_usdc = bond × twap / KASS_PRICE_SCALE (u128 intermediate,
    // overflow-checked back into u64).
    // POOL-ORIENTATION ASSUMPTION (load-bearing): `kass_price` reads the BLESSED
    // futarchy `kass_dao` spot pool, which is KASS-base / USDC-quote, so its TWAP
    // is `quote-per-base = raw-USDC per raw-KASS × KASS_PRICE_SCALE`. That is
    // exactly the "price of one KASS in USDC" we need to value a KASS bond in
    // USDC; if the pool were inverted (USDC-base/KASS-quote) this product would be
    // the reciprocal and the escrow would be nonsensical. The orientation is fixed
    // by `Protocol.kass_dao` (set once at governance handoff), so this holds for
    // every challenge under that protocol.
    assert_key(usdc_mint_ai, &oracle.usdc_mint)?;
    let protocol = load_protocol(protocol_ai, program_id)?;
    let twap = kass_price(&protocol, kass_dao_ai)?;
    let required_usdc = u64::try_from(
        (proposer.bond as u128)
            .checked_mul(twap)
            .ok_or(ProgramError::ArithmeticOverflow)?
            / KASS_PRICE_SCALE,
    )
    .map_err(|_| ProgramError::ArithmeticOverflow)?;
    // A zero escrow means the challenger stakes nothing (sub-micro KASS valuation
    // truncated to 0, or a zero bond). Reject: a challenge must put real USDC
    // skin-in-the-game, and a zero-escrow market has no source for the directional
    // USDC fee at settle. NOTE the truncation is DOWNWARD (`× twap / SCALE` floors),
    // so a funded escrow can be ≤ the exact fair value by < 1 USDC base unit —
    // settle's USDC conservation accounts for the escrow as recorded, not the ideal.
    if required_usdc == 0 {
        return Err(KassandraError::ZeroStake.into());
    }

    // --- create + fund the challenger USDC escrow vault (market-owned) ------
    // Bare SPL token account at PDA `[b"challenge_usdc", market]`, initialized
    // on the USDC mint with the oracle PDA as token authority (mirrors how
    // create_oracle stands up `stake_vault`), then funded by the challenger's
    // signed Transfer. An under-funded challenger's source account makes the
    // SPL Transfer fail, rejecting the whole instruction.
    // KNOWN LIMITATION (deferred, same mechanism as propose/submit_fact's PDA
    // creation): an attacker could grief by pre-funding this predicted escrow PDA
    // with 1 lamport so the `create_pda` CreateAccount fails. It is narrow — the
    // PDA is keyed by `market`, which is itself keyed by `ai_claim`, so it can
    // only block one specific, already-known challenge. The future fix is system
    // Allocate + Assign (tolerates a pre-funded account); not worth it now.
    let (expected_escrow, escrow_bump) = Pubkey::find_program_address(
        &[b"challenge_usdc", market_ai.address().as_ref()],
        program_id,
    );
    assert_key(escrow_vault_ai, &expected_escrow)?;
    let escrow_rent = minimum_rent(TokenAccount::LEN)?;
    let escrow_bump_seed = [escrow_bump];
    let escrow_seeds = [
        Seed::from(b"challenge_usdc".as_ref()),
        Seed::from(market_ai.address().as_ref()),
        Seed::from(&escrow_bump_seed),
    ];
    create_pda(
        challenger_ai,
        escrow_vault_ai,
        &escrow_seeds,
        escrow_rent,
        TokenAccount::LEN,
        &pinocchio_token::ID,
    )?;
    InitializeAccount3 {
        account: escrow_vault_ai,
        mint: usdc_mint_ai,
        owner: oracle_ai.address(),
    }
    .invoke()?;
    Transfer::new(
        challenger_usdc_src_ai,
        escrow_vault_ai,
        challenger_ai,
        required_usdc,
    )
    .invoke()?;

    let mut market = Market::zeroed();
    market.account_type = AccountType::Market.as_u8();
    market.oracle = *oracle_ai.address();
    market.ai_claim = *ai_claim_ai.address();
    market.proposer = *proposer_ai.address();
    market.challenger = *challenger_ai.address();
    market.question = *question_ai.address();
    market.kass_vault = *kass_vault_ai.address();
    market.usdc_vault = *usdc_vault_ai.address();
    market.pass_amm = *pass_amm_ai.address();
    market.fail_amm = *fail_amm_ai.address();
    market.oracle_pass_kass = *oracle_pass_kass_ai.address();
    market.oracle_fail_kass = *oracle_fail_kass_ai.address();
    market.challenger_usdc_vault = *escrow_vault_ai.address();
    market.twap_end = now
        .checked_add(oracle.twap_window)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    market.challenger_usdc = required_usdc;
    market.settled = 0;
    market.bump = market_bump;
    {
        let mut data = market_ai.try_borrow_mut()?;
        data.copy_from_slice(bytemuck::bytes_of(&market));
    }

    // --- flip the claim to challenged ---------------------------------------
    ai_claim.challenged = 1;
    {
        let mut data = ai_claim_ai.try_borrow_mut()?;
        data[..crate::state::AiClaim::LEN].copy_from_slice(bytemuck::bytes_of(&ai_claim));
    }

    // --- track the open challenge -------------------------------------------
    // One more market is now OPEN (not yet settled). `settle_challenge`
    // decrements this; Task 12 requires it == 0 before final plurality recompute
    // so an unsettled challenged proposer is never counted as surviving.
    oracle.open_challenge_count = oracle
        .open_challenge_count
        .checked_add(1)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    {
        let mut data = oracle_ai.try_borrow_mut()?;
        data[..Oracle::LEN].copy_from_slice(bytemuck::bytes_of(&oracle));
    }

    Ok(())
}
