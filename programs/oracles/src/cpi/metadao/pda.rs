//! PDA seed-slice assembly (host-runnable) and the SBF-only PDA derivations.

use pinocchio::address::Address as Pubkey;

use super::wire::{
    CONDITIONAL_VAULT_ID, SEED_CONDITIONAL_TOKEN, SEED_CONDITIONAL_VAULT, SEED_EVENT_AUTHORITY,
    SEED_QUESTION,
};

// ─────────────────────────────────────────────────────────────────────────────
// Seed-slice assembly (host-runnable)
// ─────────────────────────────────────────────────────────────────────────────
//
// `find_program_address` is an SBF-only syscall (it panics off-target), so the
// `*_pda` wrappers below cannot run host-side. The seed ORDER is the part most
// likely to drift, so it is factored into these tiny, host-runnable builders
// that the wrappers reuse. Tests feed the same builders into the host's
// `solana_sdk` PDA derivation (and then into the REAL program), proving the
// seed order matches the deployed binary without needing the syscall. The
// single-byte seeds (`num_outcomes`, mint `index`) are passed as `&[u8; 1]` so
// the caller owns their storage and the returned slices borrow it.

/// `Question` PDA seeds: `[b"question", question_id, oracle, [num_outcomes]]`.
pub fn question_seeds<'a>(
    question_id: &'a [u8; 32],
    oracle: &'a Pubkey,
    num_outcomes: &'a [u8; 1],
) -> [&'a [u8]; 4] {
    [SEED_QUESTION, question_id, oracle.as_ref(), num_outcomes]
}

/// `ConditionalVault` PDA seeds: `[b"conditional_vault", question, underlying_mint]`.
pub fn vault_seeds<'a>(question: &'a Pubkey, underlying_mint: &'a Pubkey) -> [&'a [u8]; 3] {
    [
        SEED_CONDITIONAL_VAULT,
        question.as_ref(),
        underlying_mint.as_ref(),
    ]
}

/// Conditional-token mint PDA seeds: `[b"conditional_token", vault, [index]]`.
pub fn conditional_token_mint_seeds<'a>(vault: &'a Pubkey, index: &'a [u8; 1]) -> [&'a [u8]; 3] {
    [SEED_CONDITIONAL_TOKEN, vault.as_ref(), index]
}

/// `#[event_cpi]` event-authority PDA seeds: `[b"__event_authority"]`.
pub fn event_authority_seeds() -> [&'static [u8]; 1] {
    [SEED_EVENT_AUTHORITY]
}

// ─────────────────────────────────────────────────────────────────────────────
// PDA derivation (SBF-only — wrap the seed builders above)
// ─────────────────────────────────────────────────────────────────────────────

/// `Question` PDA: seeds `[b"question", question_id, oracle, [num_outcomes]]`.
pub fn question_pda(question_id: &[u8; 32], oracle: &Pubkey, num_outcomes: u8) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &question_seeds(question_id, oracle, &[num_outcomes]),
        &CONDITIONAL_VAULT_ID,
    )
}

/// `ConditionalVault` PDA: seeds `[b"conditional_vault", question, underlying_mint]`.
pub fn vault_pda(question: &Pubkey, underlying_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &vault_seeds(question, underlying_mint),
        &CONDITIONAL_VAULT_ID,
    )
}

/// Conditional-token mint PDA for outcome `index`:
/// seeds `[b"conditional_token", vault, [index]]`.
pub fn conditional_token_mint_pda(vault: &Pubkey, index: u8) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &conditional_token_mint_seeds(vault, &[index]),
        &CONDITIONAL_VAULT_ID,
    )
}

/// `#[event_cpi]` event-authority PDA for `program_id`.
///
/// Parameterized by program id because each `#[event_cpi]` program (the
/// conditional_vault AND the amm) derives its own event authority under its own
/// program id. Pass [`CONDITIONAL_VAULT_ID`] for vault CPIs, [`AMM_ID`] for AMM
/// CPIs.
pub fn event_authority_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&event_authority_seeds(), program_id)
}
