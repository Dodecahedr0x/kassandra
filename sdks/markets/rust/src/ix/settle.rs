//! Settlement / teardown instruction builders (Ix 8 `ResolveMarket`,
//! Ix 9 `CollectFee`, Ix 10 `CloseMarket`).

use crate::*;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

/// `ResolveMarket` (Ix 8) — permissionless idempotent crank that bridges the
/// terminal Kassandra oracle result into the market's MetaDAO `resolve_question`.
/// The Market PDA is the resolver (it signs the CPI via seeds), so it is passed as
/// the writable `market` account AND doubles as the CPI signer.
/// Payload = empty. Accounts:
/// `[0] market(w) [1] oracle(ro) [2] question(w) [3] cv_event_authority(ro)
///  [4] cv_program(ro)`.
pub fn resolve_market(
    market: &Pubkey,
    oracle: &Pubkey,
    question: &Pubkey,
    cv_event_authority: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*market, false),
            AccountMeta::new_readonly(*oracle, false),
            AccountMeta::new(*question, false),
            AccountMeta::new_readonly(*cv_event_authority, false),
            AccountMeta::new_readonly(crate::metadao::CONDITIONAL_VAULT_ID, false),
        ],
        data: vec![IX_RESOLVE_MARKET],
    }
}

/// `CollectFee` (Ix 9) — permissionless crank that cuts the protocol `fee_bps`
/// share of a resolved market's accrued LP earnings (program-signed
/// `amm::remove_liquidity` → `conditional_vault::redeem_tokens` → SPL `transfer`)
/// into `config.fee_destination`. Payload = empty.
///
/// All addresses are derivable from `oracle` + `kass_mint` (same composition as
/// `activate`) plus the `Config` PDA's `fee_destination`.
///
/// Account order (MUST match `processor::collect_fee`):
/// ```text
///  0  market                 (w)
///  1  config                 (ro)
///  2  fee_destination        (w)
///  3  question               (ro)
///  4  vault                  (w)
///  5  vault_underlying_ata   (w)
///  6  escrow_vault           (w)
///  7  yes_mint               (w)
///  8  no_mint                (w)
///  9  market_cyes            (w)
/// 10  market_cno             (w)
/// 11  amm                    (w)
/// 12  lp_mint                (w)
/// 13  lp_vault               (w)
/// 14  amm_vault_base         (w)
/// 15  amm_vault_quote        (w)
/// 16  cv_event_authority     (ro)
/// 17  cv_program             (ro)
/// 18  amm_event_authority    (ro)
/// 19  amm_program            (ro)
/// 20  token program          (ro)
/// ```
pub fn collect_fee(
    oracle: &Pubkey,
    kass_mint: &Pubkey,
    fee_destination: &Pubkey,
    outcome_index: u8,
) -> Instruction {
    use crate::metadao as md;
    let (config, _) = crate::pda::config();
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
            AccountMeta::new_readonly(config, false),
            AccountMeta::new(*fee_destination, false),
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
        ],
        data: vec![IX_COLLECT_FEE],
    }
}

/// `CloseMarket` (Ix 10) — permissionless rent reclaim for a fully-settled market.
/// SPL-`CloseAccount`s the Market-PDA-owned token accounts (escrow always;
/// cyes/cno/lp_vault iff the market was activated) and closes the `Market` PDA, all
/// rent → `creator`. Payload = empty. Accounts:
/// `[0] market(w) [1] creator(w) [2] escrow(w) [3] cyes(w) [4] cno(w) [5] lp_vault(w)
///  [6] token program`.
///
/// All addresses are derivable from `oracle` + `outcome_index`. The cyes/cno/lp_vault
/// slots are always passed (fixed order); the program only closes them when the
/// market was activated (`market.lp_vault != default`).
pub fn close_market(oracle: &Pubkey, creator: &Pubkey, outcome_index: u8) -> Instruction {
    let (market, _) = crate::pda::market(oracle, outcome_index);
    let (escrow, _) = crate::pda::escrow(&market);
    let (cyes, _) = crate::pda::market_cyes(&market);
    let (cno, _) = crate::pda::market_cno(&market);
    let (lp_vault, _) = crate::pda::lp_vault(&market);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(market, false),
            AccountMeta::new(*creator, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new(cyes, false),
            AccountMeta::new(cno, false),
            AccountMeta::new(lp_vault, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: vec![IX_CLOSE_MARKET],
    }
}
