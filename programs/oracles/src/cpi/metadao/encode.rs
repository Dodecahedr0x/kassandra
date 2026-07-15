//! Args encoders (discriminator ++ Borsh body) and the thin invoke wrappers.

use pinocchio::{
    account::AccountView as AccountInfo,
    address::Address as Pubkey,
    cpi::Signer,
    instruction::{InstructionAccount, InstructionView},
    ProgramResult,
};

use super::wire::{
    AMM_ID, CONDITIONAL_VAULT_ID, INITIALIZE_CONDITIONAL_VAULT, INITIALIZE_QUESTION, MERGE_TOKENS,
    REDEEM_TOKENS, RESOLVE_QUESTION, SPLIT_TOKENS,
};

// ─────────────────────────────────────────────────────────────────────────────
// Args encoders (discriminator ++ Borsh body), no_std / no-alloc
// ─────────────────────────────────────────────────────────────────────────────

/// `initialize_question` instruction data.
///
/// Layout: `disc[8] ++ question_id[32] ++ oracle[32] ++ num_outcomes[1]`.
pub fn initialize_question_data(
    question_id: &[u8; 32],
    oracle: &Pubkey,
    num_outcomes: u8,
) -> [u8; 73] {
    let mut out = [0u8; 73];
    out[0..8].copy_from_slice(&INITIALIZE_QUESTION);
    out[8..40].copy_from_slice(question_id);
    out[40..72].copy_from_slice(oracle.as_ref());
    out[72] = num_outcomes;
    out
}

/// `initialize_conditional_vault` instruction data (no args).
pub fn initialize_conditional_vault_data() -> [u8; 8] {
    INITIALIZE_CONDITIONAL_VAULT
}

/// `split_tokens` instruction data. Layout: `disc[8] ++ amount[8 LE]`.
pub fn split_tokens_data(amount: u64) -> [u8; 16] {
    interact_data(&SPLIT_TOKENS, amount)
}

/// `merge_tokens` instruction data. Layout: `disc[8] ++ amount[8 LE]`.
pub fn merge_tokens_data(amount: u64) -> [u8; 16] {
    interact_data(&MERGE_TOKENS, amount)
}

/// `redeem_tokens` instruction data — NO args (just the discriminator). Validated
/// against the deployed v0.4 `conditional_vault` source: `handle_redeem_tokens`
/// takes no instruction args; it burns the holder's FULL balance of every
/// outcome's conditional token and transfers
/// `Σ_i balance_i × payout_numerators[i] / payout_denominator` underlying out of
/// the vault to the holder. For a binary pass-wins `[1,0]`: pass-balance redeems
/// 1:1, fail-balance → 0 (both burned); fail-wins `[0,1]` is symmetric. Uses the
/// SAME `InteractWithVault` account struct as `split_tokens` (see the account
/// ordering note below).
pub fn redeem_tokens_data() -> [u8; 8] {
    REDEEM_TOKENS
}

fn interact_data(disc: &[u8; 8], amount: u64) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0..8].copy_from_slice(disc);
    out[8..16].copy_from_slice(&amount.to_le_bytes());
    out
}

/// `resolve_question` instruction data for a BINARY (2-outcome) question.
///
/// Layout: `disc[8] ++ len:u32 LE (== 2) ++ payout_numerators[0]:u32 LE ++
/// payout_numerators[1]:u32 LE`. The arg is Anchor `ResolveQuestionArgs {
/// payout_numerators: Vec<u32> }`, and a Borsh `Vec<u32>` is a **4-byte LE
/// length prefix THEN the u32 elements** — NOT a flat concatenation. No-alloc:
/// the whole thing is a fixed 20-byte buffer.
///
/// `[1, 0]` resolves PASS-side (outcome 0 pays); `[0, 1]` resolves FAIL-side
/// (outcome 1 pays). The conditional_vault requires `len == num_outcomes` and a
/// non-zero payout denominator (sum of numerators), so exactly one of the two
/// must be `1`.
pub fn resolve_question_data_binary(numerators: [u32; 2]) -> [u8; 20] {
    let mut out = [0u8; 20];
    out[0..8].copy_from_slice(&RESOLVE_QUESTION);
    out[8..12].copy_from_slice(&2u32.to_le_bytes());
    out[12..16].copy_from_slice(&numerators[0].to_le_bytes());
    out[16..20].copy_from_slice(&numerators[1].to_le_bytes());
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Account orderings (for the program-side CPI, Task 10)
// ─────────────────────────────────────────────────────────────────────────────
//
// `initialize_question` — accounts:
//   0 question (w, PDA, init)   1 payer (signer, w)   2 system_program
//   3 event_authority           4 conditional_vault program id
//
// `initialize_conditional_vault` — accounts:
//   0 vault (w, PDA, init)      1 question              2 underlying_token_mint
//   3 vault_underlying_ata (w, init_if_needed)          4 payer (signer, w)
//   5 token_program             6 associated_token_program   7 system_program
//   8 event_authority           9 conditional_vault program id
//   …remaining: conditional_token_mint[0..num_outcomes] (w, PDA, created here)
//
// `split_tokens` / `merge_tokens` / `redeem_tokens` — accounts (InteractWithVault):
//   0 question                  1 vault (w)             2 vault_underlying_ata (w)
//   3 authority (signer)        4 user_underlying_ata (w)   5 token_program
//   6 event_authority           7 conditional_vault program id
//   …remaining: conditional_token_mint[0..n] (w)
//              then user_conditional_token_account[0..n] (w, owner == authority)
//
// All THREE share the identical `InteractWithVault` account struct (verified
// against the deployed v0.4 `conditional_vault` source `common.rs`); only the
// handler differs. `user_underlying_token_account` is constrained
// `token::authority = authority` + `token::mint = vault.underlying_token_mint`,
// so on a program-signed `redeem_tokens` the redeemed underlying lands in an
// account owned by the signing authority (our oracle PDA — i.e. `stake_vault`),
// and the `user_conditional_token_account[i]` must be owned by that same
// authority. `redeem_tokens` additionally requires `question.is_resolved()`.
//
// For split, the vault mints `amount` of EACH outcome's conditional token to the
// user and pulls `amount` underlying into the vault ATA. Binary (pass/fail)
// markets use num_outcomes == 2; outcome index → conditional_token_mint index.

// ─────────────────────────────────────────────────────────────────────────────
// Thin invoke wrappers
// ─────────────────────────────────────────────────────────────────────────────

/// Invoke an instruction on the `conditional_vault` program.
///
/// `metas` must be in the order documented above (including the two trailing
/// `#[event_cpi]` accounts and any remaining accounts); `infos` must be the
/// matching `AccountInfo`s in the same order. `data` is a discriminator-prefixed
/// payload from the encoders above. Pass PDA `signers` when our program must
/// authorize a split/merge of vault-held KASS.
pub fn invoke_conditional_vault_signed<A: AsRef<AccountInfo>>(
    data: &[u8],
    metas: &[InstructionAccount],
    infos: &[A],
    signers: &[Signer],
) -> ProgramResult {
    let ix = InstructionView {
        program_id: &CONDITIONAL_VAULT_ID,
        data,
        accounts: metas,
    };
    pinocchio::cpi::invoke_signed_with_slice(&ix, infos, signers)
}

/// Invoke an instruction on the `amm` program (Task 10/11 wiring).
pub fn invoke_amm_signed<A: AsRef<AccountInfo>>(
    data: &[u8],
    metas: &[InstructionAccount],
    infos: &[A],
    signers: &[Signer],
) -> ProgramResult {
    let ix = InstructionView {
        program_id: &AMM_ID,
        data,
        accounts: metas,
    };
    pinocchio::cpi::invoke_signed_with_slice(&ix, infos, signers)
}
