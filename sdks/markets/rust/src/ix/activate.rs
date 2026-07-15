//! Activation instruction builders (Ix 6 `Activate`, Ix 7 `ClaimLp`).

use crate::*;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

/// `Activate` (Ix 6) — turn a fully-funded `Funding` market into a live MetaDAO
/// cYES/cNO AMM market: verify the client-composed MetaDAO market, program-signed
/// split the escrowed KASS into cYES/cNO, and seed the AMM pool 50/50.
///
/// All addresses are derivable from `oracle` + `kass_mint` (the MetaDAO market was
/// composed with `oracle_authority = market PDA`, `question_id = oracle bytes`,
/// underlying `= kass_mint`, `base = cYES`, `quote = cNO`). Payload = empty.
///
/// Account order (MUST match `processor::activate`):
/// ```text
///  0  market                 (w)  — the market PDA, must be `Funding`
///  1  oracle                 (ro) — kassandra oracle, non-terminal
///  2  payer                  (signer,w) — rent for the 3 new market-owned token accts
///  3  question               (ro) — MetaDAO Question (oracle-authority == market)
///  4  vault                  (w)  — KASS conditional vault
///  5  vault_underlying_ata   (w)  — vault's KASS ATA (split destination for underlying)
///  6  escrow_vault           (w)  — market.escrow_vault (split source)
///  7  yes_mint               (w)  — conditional mint idx 0 (cYES)
///  8  no_mint                (w)  — conditional mint idx 1 (cNO)
///  9  market_cyes            (w)  — market-PDA-owned cYES holder (created here)
/// 10  market_cno             (w)  — market-PDA-owned cNO holder (created here)
/// 11  amm                    (w)  — the cYES/cNO pool
/// 12  lp_mint                (w)  — the pool's LP mint
/// 13  lp_vault               (w)  — market-PDA-owned LP holder (created here)
/// 14  amm_vault_base         (w)  — amm's cYES ATA
/// 15  amm_vault_quote        (w)  — amm's cNO ATA
/// 16  cv_event_authority     (ro)
/// 17  cv_program             (ro)
/// 18  amm_event_authority    (ro)
/// 19  amm_program            (ro)
/// 20  token program          (ro)
/// 21  system program         (ro)
/// ```
pub fn activate(
    payer: &Pubkey,
    oracle: &Pubkey,
    kass_mint: &Pubkey,
    outcome_index: u8,
) -> Instruction {
    use crate::metadao as md;
    let (market, _) = crate::pda::market(oracle, outcome_index);
    let (escrow, _) = crate::pda::escrow(&market);
    let (question, _) = md::question(&oracle.to_bytes(), &market, 2);
    let (vault, _) = md::vault(&question, kass_mint);
    let vault_underlying_ata = md::ata(&vault, kass_mint);
    let (yes_mint, _) = md::conditional_token_mint(&vault, 0);
    let (no_mint, _) = md::conditional_token_mint(&vault, 1);
    let (market_cyes, _) = crate::pda::market_cyes(&market);
    let (market_cno, _) = crate::pda::market_cno(&market);
    let (amm, _) = md::amm(&yes_mint, &no_mint);
    let (lp_mint, _) = md::amm_lp_mint(&amm);
    let (lp_vault, _) = crate::pda::lp_vault(&market);
    let amm_vault_base = md::ata(&amm, &yes_mint);
    let amm_vault_quote = md::ata(&amm, &no_mint);
    let (cv_event_auth, _) = md::event_authority(&md::CONDITIONAL_VAULT_ID);
    let (amm_event_auth, _) = md::event_authority(&md::AMM_ID);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(market, false),
            AccountMeta::new_readonly(*oracle, false),
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(question, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_underlying_ata, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new(yes_mint, false),
            AccountMeta::new(no_mint, false),
            AccountMeta::new(market_cyes, false),
            AccountMeta::new(market_cno, false),
            AccountMeta::new(amm, false),
            AccountMeta::new(lp_mint, false),
            AccountMeta::new(lp_vault, false),
            AccountMeta::new(amm_vault_base, false),
            AccountMeta::new(amm_vault_quote, false),
            AccountMeta::new_readonly(cv_event_auth, false),
            AccountMeta::new_readonly(md::CONDITIONAL_VAULT_ID, false),
            AccountMeta::new_readonly(amm_event_auth, false),
            AccountMeta::new_readonly(md::AMM_ID, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: vec![IX_ACTIVATE],
    }
}

/// `ClaimLp` (Ix 7) — permissionless per-contributor claim of the AMM LP tokens
/// seeded at `activate`, out of the Market-PDA-owned `lp_vault`. Program-signed
/// transfer of the floor pro-rata share (or the ENTIRE remaining `lp_vault` for the
/// LAST claimer) to the recorded contributor's LP token account, then the
/// `Contribution` is CLOSED with its rent returned to `contributor`.
/// Payload = empty. Accounts:
/// `[0] market(w) [1] lp_vault(w) [2] contribution(w) [3] contributor_lp_ata(w)
///  [4] contributor(w) [5] token program`.
///
/// `market` is writable (its `open_contributions` counter is decremented) and
/// `contributor` (== `contribution.contributor`) receives the closed Contribution's
/// rent.
pub fn claim_lp(
    market: &Pubkey,
    lp_vault: &Pubkey,
    contribution: &Pubkey,
    contributor_lp_ata: &Pubkey,
    contributor: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*market, false),
            AccountMeta::new(*lp_vault, false),
            AccountMeta::new(*contribution, false),
            AccountMeta::new(*contributor_lp_ata, false),
            AccountMeta::new(*contributor, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: vec![IX_CLAIM_LP],
    }
}
