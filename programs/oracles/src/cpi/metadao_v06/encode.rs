//! Args encoders (discriminator ++ Borsh body) and the thin invoke wrappers.

use pinocchio::{
    account::AccountView as AccountInfo,
    cpi::Signer,
    instruction::{InstructionAccount, InstructionView},
    ProgramResult,
};

use super::wire::{
    FUTARCHY_ID, FUT_FINALIZE_PROPOSAL, FUT_INITIALIZE_DAO, FUT_INITIALIZE_PROPOSAL,
    METEORA_DAMM_V2_ID,
};

// ─────────────────────────────────────────────────────────────────────────────
// Args encoders (discriminator ++ Borsh body), no_std / no-alloc
// ─────────────────────────────────────────────────────────────────────────────
//
// STUBBED (documented, not yet wire-validated): `initialize_dao` takes
// `InitializeDaoParams { twap_initial_observation:u128,
// twap_max_observation_change_per_update:u128, twap_start_delay_seconds:u32,
// min_quote_futarchic_liquidity:u64, min_base_futarchic_liquidity:u64,
// base_to_stake:u64, pass_threshold_bps:u16, seconds_per_proposal:u32, nonce:u64,
// initial_spending_limit: Option<InitialSpendingLimit> }` (Borsh). The trailing
// `Option<Vec<Pubkey>>` makes it variable-length; F6 builds it. The fixed-size
// prefix is encoded below for the common `None` spending-limit case.

/// `initialize_proposal` instruction data (no positional args).
pub fn initialize_proposal_data() -> [u8; 8] {
    FUT_INITIALIZE_PROPOSAL
}

/// `finalize_proposal` instruction data (no positional args).
pub fn finalize_proposal_data() -> [u8; 8] {
    FUT_FINALIZE_PROPOSAL
}

/// `initialize_dao` instruction data for the `initial_spending_limit == None`
/// case. Layout: `disc[8] ++ twap_initial_observation:u128 ++
/// twap_max_observation_change_per_update:u128 ++ twap_start_delay_seconds:u32 ++
/// min_quote_futarchic_liquidity:u64 ++ min_base_futarchic_liquidity:u64 ++
/// base_to_stake:u64 ++ pass_threshold_bps:u16 ++ seconds_per_proposal:u32 ++
/// nonce:u64 ++ 0u8 (Option::None tag)` = 8+16+16+4+8+8+8+2+4+8+1 = 83 bytes.
#[allow(clippy::too_many_arguments)]
pub fn initialize_dao_data_no_limit(
    twap_initial_observation: u128,
    twap_max_observation_change_per_update: u128,
    twap_start_delay_seconds: u32,
    min_quote_futarchic_liquidity: u64,
    min_base_futarchic_liquidity: u64,
    base_to_stake: u64,
    pass_threshold_bps: u16,
    seconds_per_proposal: u32,
    nonce: u64,
) -> [u8; 83] {
    let mut out = [0u8; 83];
    let mut o = 0usize;
    let put = |bytes: &[u8], out: &mut [u8; 83], o: &mut usize| {
        out[*o..*o + bytes.len()].copy_from_slice(bytes);
        *o += bytes.len();
    };
    put(&FUT_INITIALIZE_DAO, &mut out, &mut o);
    put(&twap_initial_observation.to_le_bytes(), &mut out, &mut o);
    put(
        &twap_max_observation_change_per_update.to_le_bytes(),
        &mut out,
        &mut o,
    );
    put(&twap_start_delay_seconds.to_le_bytes(), &mut out, &mut o);
    put(
        &min_quote_futarchic_liquidity.to_le_bytes(),
        &mut out,
        &mut o,
    );
    put(
        &min_base_futarchic_liquidity.to_le_bytes(),
        &mut out,
        &mut o,
    );
    put(&base_to_stake.to_le_bytes(), &mut out, &mut o);
    put(&pass_threshold_bps.to_le_bytes(), &mut out, &mut o);
    put(&seconds_per_proposal.to_le_bytes(), &mut out, &mut o);
    put(&nonce.to_le_bytes(), &mut out, &mut o);
    // initial_spending_limit: Option::None
    put(&[0u8], &mut out, &mut o);
    debug_assert_eq!(o, 83);
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Thin invoke wrappers
// ─────────────────────────────────────────────────────────────────────────────

/// Invoke an instruction on the `futarchy` v0.6 program with PDA signers.
pub fn invoke_futarchy_signed<A: AsRef<AccountInfo>>(
    data: &[u8],
    metas: &[InstructionAccount],
    infos: &[A],
    signers: &[Signer],
) -> ProgramResult {
    let ix = InstructionView {
        program_id: &FUTARCHY_ID,
        data,
        accounts: metas,
    };
    pinocchio::cpi::invoke_signed_with_slice(&ix, infos, signers)
}

/// Invoke an instruction on the Meteora DAMM v2 (cp-amm) program with PDA signers.
pub fn invoke_meteora_signed<A: AsRef<AccountInfo>>(
    data: &[u8],
    metas: &[InstructionAccount],
    infos: &[A],
    signers: &[Signer],
) -> ProgramResult {
    let ix = InstructionView {
        program_id: &METEORA_DAMM_V2_ID,
        data,
        accounts: metas,
    };
    pinocchio::cpi::invoke_signed_with_slice(&ix, infos, signers)
}
