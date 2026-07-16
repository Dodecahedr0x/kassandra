//! Add-liquidity instruction builder (Ix 11 `AddLiquidity`).

use crate::*;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

/// `AddLiquidity` (Ix 11) — deposit `amount` KASS into an already-`Active`
/// market's live cYES/cNO AMM, minting pooled LP into the Market-PDA-owned
/// `lp_vault` (claimable pro-rata alongside the funders). Program-signed
/// `split_tokens` + `add_liquidity` mirror `activate`; the ratio-limited remainder
/// is returned to the depositor's cYES/cNO ATA.
///
/// All addresses derive from `oracle` + `kass_mint` + `depositor`. The depositor's
/// KASS/cYES/cNO accounts are the canonical ATAs (the cYES/cNO ATAs must exist to
/// receive the returned remainder — create them idempotently client-side).
///
/// `quote_amount`/`max_base_amount` are computed by the caller from the live pool
/// reserves: `quote_amount = min(amount, floor(amount · quoteReserve /
/// baseReserve))`, `max_base_amount = amount` (base = cYES, quote = cNO), so
/// neither side needs more than the `amount` that was split. `min_lp_tokens` is
/// the caller's slippage floor — MetaDAO REQUIRES it to be non-zero for a
/// non-empty pool. Payload = `amount ++ quote_amount ++ max_base_amount ++
/// min_lp_tokens` (4 × u64 LE).
///
/// Account order (MUST match `processor::add_liquidity`):
/// ```text
///  0  market                (w)        — must be `Active`
///  1  oracle                (ro)       — non-terminal
///  2  depositor             (signer,w) — KASS source authority + contribution rent
///  3  depositor_kass_ata    (w)        — split-funding source (KASS)
///  4  escrow_vault          (w)        — market.escrow_vault
///  5  question              (ro)
///  6  vault                 (w)
///  7  vault_underlying_ata  (w)
///  8  yes_mint              (w)
///  9  no_mint               (w)
/// 10  market_cyes           (w)        — [b"cyes", market]
/// 11  market_cno            (w)        — [b"cno", market]
/// 12  depositor_cyes_ata    (w)        — remainder return dest (cYES)
/// 13  depositor_cno_ata     (w)        — remainder return dest (cNO)
/// 14  amm                   (w)
/// 15  lp_mint               (w)
/// 16  lp_vault              (w)        — user_lp (LP minted here)
/// 17  amm_vault_base        (w)
/// 18  amm_vault_quote       (w)
/// 19  contribution          (w)        — [b"contribution", market, depositor]
/// 20  cv_event_authority    (ro)
/// 21  cv_program            (ro)
/// 22  amm_event_authority   (ro)
/// 23  amm_program           (ro)
/// 24  token program         (ro)
/// 25  system program        (ro)
/// ```
#[allow(clippy::too_many_arguments)]
pub fn add_liquidity(
    depositor: &Pubkey,
    oracle: &Pubkey,
    kass_mint: &Pubkey,
    outcome_index: u8,
    amount: u64,
    quote_amount: u64,
    max_base_amount: u64,
    min_lp_tokens: u64,
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
    let depositor_kass_ata = md::ata(depositor, kass_mint);
    let depositor_cyes_ata = md::ata(depositor, &yes_mint);
    let depositor_cno_ata = md::ata(depositor, &no_mint);
    let (amm, _) = md::amm(&yes_mint, &no_mint);
    let (lp_mint, _) = md::amm_lp_mint(&amm);
    let (lp_vault, _) = crate::pda::lp_vault(&market);
    let amm_vault_base = md::ata(&amm, &yes_mint);
    let amm_vault_quote = md::ata(&amm, &no_mint);
    let (contribution, _) = crate::pda::contribution(&market, depositor);
    let (cv_event_auth, _) = md::event_authority(&md::CONDITIONAL_VAULT_ID);
    let (amm_event_auth, _) = md::event_authority(&md::AMM_ID);

    let mut data = Vec::with_capacity(33);
    data.push(IX_ADD_LIQUIDITY);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&quote_amount.to_le_bytes());
    data.extend_from_slice(&max_base_amount.to_le_bytes());
    data.extend_from_slice(&min_lp_tokens.to_le_bytes());

    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(market, false),
            AccountMeta::new_readonly(*oracle, false),
            AccountMeta::new(*depositor, true),
            AccountMeta::new(depositor_kass_ata, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new_readonly(question, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(vault_underlying_ata, false),
            AccountMeta::new(yes_mint, false),
            AccountMeta::new(no_mint, false),
            AccountMeta::new(market_cyes, false),
            AccountMeta::new(market_cno, false),
            AccountMeta::new(depositor_cyes_ata, false),
            AccountMeta::new(depositor_cno_ata, false),
            AccountMeta::new(amm, false),
            AccountMeta::new(lp_mint, false),
            AccountMeta::new(lp_vault, false),
            AccountMeta::new(amm_vault_base, false),
            AccountMeta::new(amm_vault_quote, false),
            AccountMeta::new(contribution, false),
            AccountMeta::new_readonly(cv_event_auth, false),
            AccountMeta::new_readonly(md::CONDITIONAL_VAULT_ID, false),
            AccountMeta::new_readonly(amm_event_auth, false),
            AccountMeta::new_readonly(md::AMM_ID, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    }
}
