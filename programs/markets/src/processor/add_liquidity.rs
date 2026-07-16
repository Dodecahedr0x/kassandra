//! `add_liquidity` (Ix 11): deposit KASS into an already-`Active` market's live
//! cYES/cNO AMM, minting pooled LP into the Market-PDA-owned `lp_vault` so it is
//! claimable pro-rata alongside the original funders.
//!
//! # Flow (program-signed, mirroring `activate`)
//! The Market PDA is the authority for both MetaDAO CPIs, using the same account
//! wiring `activate` already proves out:
//! 1. Depositor-signed SPL `Transfer` of `amount` KASS → `escrow_vault`.
//! 2. Program-signed `conditional_vault::split_tokens(amount)`: `escrow_vault →
//!    market_cyes`/`market_cno` (drains escrow back to its prior residual).
//! 3. Program-signed `amm::add_liquidity(quote_amount, max_base_amount, 0)` at the
//!    pool's *current* ratio: deposits the ratio-limited amounts from
//!    `market_cyes`/`market_cno`, mints LP into `lp_vault`.
//! 4. Program-signed SPL `Transfer` of the leftover heavy-side balance from
//!    `market_cyes`/`market_cno` back to the depositor's cYES/cNO ATA, returning
//!    both transient holders to 0 (preserving `collect_fee`'s "empty since
//!    activate" assumption).
//!
//! `quote_amount`/`max_base_amount` are client-computed from the live reserves so
//! neither side ever needs more than the `amount` that was split; MetaDAO enforces
//! the ratio, so a mis-computed hint can only enlarge the returned remainder or
//! revert — never move funds incorrectly.
//!
//! # Accounting (gross-LP basis — see the design doc)
//! `lp_new` = the `lp_vault` balance delta. `lp_total += lp_new`,
//! `gross_lp_total += lp_new`, `total_contributed += amount` (conservative), and
//! the depositor's `Contribution.late_lp += lp_new` (created if absent, bumping
//! `open_contributions`). `claim_lp` then pays this out by gross LP.
//!
//! # Instruction payload (after the 1-byte discriminant), exactly 32 bytes
//! `amount ++ quote_amount ++ max_base_amount ++ min_lp_tokens` (4 × u64 LE).
//! MetaDAO's `add_liquidity` REQUIRES a non-zero `min_lp_tokens` when adding to a
//! non-empty pool (its slippage guard), so the client supplies it.

use bytemuck::Zeroable;
use pinocchio::{
    account::AccountView, address::Address, cpi::Seed, cpi::Signer, error::ProgramError,
    instruction::InstructionAccount, ProgramResult,
};
use pinocchio_token::instructions::Transfer;

use crate::{
    cpi::metadao,
    cpi::spl::SPL_TOKEN_AMOUNT_OFFSET,
    error::MarketError,
    processor::guards::{
        assert_key, assert_owned_by_program, assert_signer, create_pda, load_contribution,
        load_kassandra_oracle, load_market, market_signer_seeds, rent_exempt_lamports,
        write_contribution, write_market,
    },
    state::{AccountType, Contribution, MarketStatus},
};

pub fn process(
    program_id: &Address,
    accounts: &mut [AccountView],
    payload: &[u8],
) -> ProgramResult {
    if payload.len() != 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let amount = u64::from_le_bytes(payload[0..8].try_into().unwrap());
    let quote_amount = u64::from_le_bytes(payload[8..16].try_into().unwrap());
    let max_base_amount = u64::from_le_bytes(payload[16..24].try_into().unwrap());
    let min_lp_tokens = u64::from_le_bytes(payload[24..32].try_into().unwrap());
    if amount == 0 {
        return Err(MarketError::ZeroAmount.into());
    }

    let [market_ai, oracle_ai, depositor_ai, depositor_kass_ai, escrow_ai, question_ai, vault_ai, vault_underlying_ai, yes_mint_ai, no_mint_ai, market_cyes_ai, market_cno_ai, depositor_cyes_ai, depositor_cno_ai, amm_ai, lp_mint_ai, lp_vault_ai, amm_vault_base_ai, amm_vault_quote_ai, contribution_ai, cv_event_auth_ai, cv_prog_ai, amm_event_auth_ai, amm_prog_ai, token_prog_ai, system_prog_ai, ..] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // --- signer + program ids ----------------------------------------------
    assert_signer(depositor_ai)?;
    assert_key(cv_prog_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    assert_key(amm_prog_ai, &metadao::AMM_ID)?;
    assert_key(token_prog_ai, &pinocchio_token::ID)?;
    assert_key(system_prog_ai, &pinocchio_system::ID)?;

    // --- market state gate: must be Active ----------------------------------
    let market = load_market(market_ai, program_id)?;
    if market.status != MarketStatus::Active.as_u8() {
        return Err(MarketError::NotActive.into());
    }

    // --- oracle must be NON-terminal ----------------------------------------
    // Once the oracle can resolve, no new liquidity (mirror `activate`): a market
    // about to settle must not take deposits it can't fairly place.
    assert_key(oracle_ai, &market.oracle)?;
    let oracle = load_kassandra_oracle(oracle_ai)?;
    let terminal = oracle.phase == crate::kass_oracle::PHASE_RESOLVED
        || oracle.phase == crate::kass_oracle::PHASE_INVALID_DEADEND;
    if terminal {
        return Err(MarketError::OracleResolved.into());
    }

    // --- verify the recorded MetaDAO bindings (mirror `collect_fee`) ---------
    assert_key(escrow_ai, &market.escrow_vault)?;
    assert_key(question_ai, &market.question)?;
    assert_owned_by_program(question_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    assert_key(vault_ai, &market.vault)?;
    assert_owned_by_program(vault_ai, &metadao::CONDITIONAL_VAULT_ID)?;
    assert_key(yes_mint_ai, &market.yes_mint)?;
    assert_key(no_mint_ai, &market.no_mint)?;
    assert_key(amm_ai, &market.amm)?;
    assert_owned_by_program(amm_ai, &metadao::AMM_ID)?;
    assert_key(lp_mint_ai, &market.lp_mint)?;
    assert_key(lp_vault_ai, &market.lp_vault)?;

    // The vault's underlying (KASS) ATA + mint binding.
    {
        let d = vault_ai.try_borrow()?;
        let v_underlying = metadao::read_pubkey(&d, metadao::VAULT_UNDERLYING_MINT_OFFSET)?;
        let v_underlying_acct = metadao::read_pubkey(&d, metadao::VAULT_UNDERLYING_ACCOUNT_OFFSET)?;
        if v_underlying != market.kass_mint || &v_underlying_acct != vault_underlying_ai.address() {
            return Err(MarketError::InvalidAccount.into());
        }
    }

    // The two transient Market-PDA-owned cYES/cNO holders (created at `activate`).
    let (expect_cyes, _) =
        Address::find_program_address(&[b"cyes", market_ai.address().as_ref()], program_id);
    let (expect_cno, _) =
        Address::find_program_address(&[b"cno", market_ai.address().as_ref()], program_id);
    assert_key(market_cyes_ai, &expect_cyes)?;
    assert_key(market_cno_ai, &expect_cno)?;

    // The AMM per-mint vault ATAs + event authorities.
    let (expect_vault_base, _) =
        metadao::associated_token_address(amm_ai.address(), yes_mint_ai.address());
    let (expect_vault_quote, _) =
        metadao::associated_token_address(amm_ai.address(), no_mint_ai.address());
    assert_key(amm_vault_base_ai, &expect_vault_base)?;
    assert_key(amm_vault_quote_ai, &expect_vault_quote)?;
    let (cv_event_auth, _) = metadao::event_authority_pda(&metadao::CONDITIONAL_VAULT_ID);
    assert_key(cv_event_auth_ai, &cv_event_auth)?;
    let (amm_event_auth, _) = metadao::event_authority_pda(&metadao::AMM_ID);
    assert_key(amm_event_auth_ai, &amm_event_auth)?;

    // The leftover-return destinations must be the depositor's own cYES/cNO ATAs.
    let (expect_dep_cyes, _) =
        metadao::associated_token_address(depositor_ai.address(), yes_mint_ai.address());
    let (expect_dep_cno, _) =
        metadao::associated_token_address(depositor_ai.address(), no_mint_ai.address());
    assert_key(depositor_cyes_ai, &expect_dep_cyes)?;
    assert_key(depositor_cno_ai, &expect_dep_cno)?;

    // --- (1) depositor-signed KASS transfer into escrow ---------------------
    Transfer::new(depositor_kass_ai, escrow_ai, depositor_ai, amount).invoke()?;

    // --- market-PDA signer seeds (shared by every program-signed CPI) -------
    market_signer_seeds!(market, oidx, mbump, market_seeds);

    // --- (2) program-signed split: escrow KASS -> cYES/cNO ------------------
    let split_data = metadao::split_tokens_data(amount);
    let split_metas = [
        InstructionAccount::readonly(question_ai.address()),
        InstructionAccount::writable(vault_ai.address()),
        InstructionAccount::writable(vault_underlying_ai.address()),
        InstructionAccount::readonly_signer(market_ai.address()), // authority (market PDA)
        InstructionAccount::writable(escrow_ai.address()),        // user_underlying (split source)
        InstructionAccount::readonly(token_prog_ai.address()),
        InstructionAccount::readonly(cv_event_auth_ai.address()),
        InstructionAccount::readonly(cv_prog_ai.address()),
        InstructionAccount::writable(yes_mint_ai.address()),
        InstructionAccount::writable(no_mint_ai.address()),
        InstructionAccount::writable(market_cyes_ai.address()),
        InstructionAccount::writable(market_cno_ai.address()),
    ];
    let split_infos = [
        &*question_ai,
        &*vault_ai,
        &*vault_underlying_ai,
        &*market_ai,
        &*escrow_ai,
        &*token_prog_ai,
        &*cv_event_auth_ai,
        &*cv_prog_ai,
        &*yes_mint_ai,
        &*no_mint_ai,
        &*market_cyes_ai,
        &*market_cno_ai,
    ];
    metadao::invoke_conditional_vault_signed(
        &split_data,
        &split_metas,
        &split_infos,
        &[Signer::from(&market_seeds)],
    )?;

    // --- LP balance before the add (delta = LP minted to us) ----------------
    let lp_before = {
        let d = lp_vault_ai.try_borrow()?;
        metadao::read_u64(&d, SPL_TOKEN_AMOUNT_OFFSET)?
    };

    // --- (3) program-signed add_liquidity at the live ratio -----------------
    let add_data = metadao::add_liquidity_data(quote_amount, max_base_amount, min_lp_tokens);
    let add_metas = [
        InstructionAccount::writable_signer(market_ai.address()), // authority (market PDA)
        InstructionAccount::writable(amm_ai.address()),
        InstructionAccount::writable(lp_mint_ai.address()),
        InstructionAccount::writable(lp_vault_ai.address()), // user_lp
        InstructionAccount::writable(market_cyes_ai.address()), // user_base
        InstructionAccount::writable(market_cno_ai.address()), // user_quote
        InstructionAccount::writable(amm_vault_base_ai.address()),
        InstructionAccount::writable(amm_vault_quote_ai.address()),
        InstructionAccount::readonly(token_prog_ai.address()),
        InstructionAccount::readonly(amm_event_auth_ai.address()),
        InstructionAccount::readonly(amm_prog_ai.address()),
    ];
    let add_infos = [
        &*market_ai,
        &*amm_ai,
        &*lp_mint_ai,
        &*lp_vault_ai,
        &*market_cyes_ai,
        &*market_cno_ai,
        &*amm_vault_base_ai,
        &*amm_vault_quote_ai,
        &*token_prog_ai,
        &*amm_event_auth_ai,
        &*amm_prog_ai,
    ];
    metadao::invoke_amm_signed(
        &add_data,
        &add_metas,
        &add_infos,
        &[Signer::from(&market_seeds)],
    )?;

    // --- LP minted to us ----------------------------------------------------
    let lp_after = {
        let d = lp_vault_ai.try_borrow()?;
        metadao::read_u64(&d, SPL_TOKEN_AMOUNT_OFFSET)?
    };
    let lp_new = lp_after
        .checked_sub(lp_before)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    if lp_new == 0 {
        // A no-op add (no LP minted) would still have consumed the split with nothing
        // to show for it — reject so the whole tx unwinds and the depositor keeps
        // their KASS.
        return Err(MarketError::InvalidAccount.into());
    }

    // --- (4) return the leftover heavy-side cYES/cNO to the depositor -------
    // add_liquidity deposits only the ratio-limited amounts; whatever the split left
    // in the transient holders is the remainder. Sweeping both back to the depositor
    // returns market_cyes/market_cno to 0.
    return_leftover(market_cyes_ai, depositor_cyes_ai, market_ai, &market_seeds)?;
    return_leftover(market_cno_ai, depositor_cno_ai, market_ai, &market_seeds)?;

    // --- record the contribution (create-or-increment `late_lp`) ------------
    let creates_new = contribution_ai.lamports() == 0 && contribution_ai.is_data_empty();
    record_late_lp(
        program_id,
        market_ai.address(),
        depositor_ai,
        contribution_ai,
        lp_new,
    )?;

    // --- update market accounting -------------------------------------------
    let mut m = market;
    m.lp_total = m
        .lp_total
        .checked_add(lp_new)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    m.gross_lp_total = m
        .gross_lp_total
        .checked_add(lp_new)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    // Conservative: the returned remainder is one-sided conditional tokens with no
    // clean KASS value until resolution, so we credit the FULL split. This can only
    // under-state `accrued` in `collect_fee` (under-collect the protocol fee) — never
    // a safety issue, and it does not touch claim fairness (claims use gross LP).
    m.total_contributed = m
        .total_contributed
        .checked_add(amount)
        .ok_or(ProgramError::ArithmeticOverflow)?;
    if creates_new {
        m.open_contributions = m
            .open_contributions
            .checked_add(1)
            .ok_or(ProgramError::ArithmeticOverflow)?;
    }
    write_market(market_ai, &m)?;

    Ok(())
}

/// Sweep the full balance of a Market-PDA-owned transient token holder back to the
/// depositor's ATA (program-signed with the market seeds). No-op when empty.
fn return_leftover(
    from_ai: &AccountView,
    to_ai: &AccountView,
    market_ai: &AccountView,
    market_seeds: &[Seed],
) -> ProgramResult {
    let bal = {
        let d = from_ai.try_borrow()?;
        metadao::read_u64(&d, SPL_TOKEN_AMOUNT_OFFSET)?
    };
    if bal > 0 {
        Transfer::new(from_ai, to_ai, market_ai, bal)
            .invoke_signed(&[Signer::from(market_seeds)])?;
    }
    Ok(())
}

/// Create-or-increment the depositor's `Contribution`, adding `lp_new` to `late_lp`
/// (KASS `amount` untouched — the escrow funding here backs LP, not a funding
/// stake). Mirrors `record_contribution`'s create/adopt shape without the KASS move.
fn record_late_lp(
    program_id: &Address,
    market_key: &Address,
    depositor_ai: &AccountView,
    contribution_ai: &mut AccountView,
    lp_new: u64,
) -> ProgramResult {
    let (expected, bump) = Address::find_program_address(
        &[
            b"contribution",
            market_key.as_ref(),
            depositor_ai.address().as_ref(),
        ],
        program_id,
    );
    assert_key(contribution_ai, &expected)?;

    if contribution_ai.lamports() == 0 && contribution_ai.is_data_empty() {
        let rent = rent_exempt_lamports(Contribution::LEN)?;
        let bump_seed = [bump];
        let seeds = [
            Seed::from(b"contribution".as_ref()),
            Seed::from(market_key.as_ref()),
            Seed::from(depositor_ai.address().as_ref()),
            Seed::from(&bump_seed),
        ];
        create_pda(
            depositor_ai,
            contribution_ai,
            &seeds,
            rent,
            Contribution::LEN,
            program_id,
        )?;
        let mut c = Contribution::zeroed();
        c.account_type = AccountType::Contribution.as_u8();
        c.market = *market_key;
        c.contributor = *depositor_ai.address();
        c.bump = bump;
        c.late_lp = lp_new;
        write_contribution(contribution_ai, &c)?;
    } else {
        let mut c = load_contribution(contribution_ai, program_id)?;
        c.late_lp = c
            .late_lp
            .checked_add(lp_new)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        write_contribution(contribution_ai, &c)?;
    }
    Ok(())
}
