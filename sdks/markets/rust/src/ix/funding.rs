//! Funding-phase instruction builders (Ix 2 `CreateMarket`, Ix 3 `Contribute`,
//! Ix 4 `Cancel`, Ix 5 `Refund`).

use crate::*;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

/// `CreateMarket` (Ix 2) — create the `outcome_index` binary sub-market for
/// `oracle`, its KASS escrow, and the creator's `Contribution`, transferring
/// `seed_amount` KASS in.
/// Payload = `seed_amount` (u64 LE) ++ `outcome_index` (u8). Accounts:
/// `[0] config(ro) [1] oracle(ro) [2] market(pda,w) [3] escrow(pda,w)
///  [4] kass_mint(ro) [5] creator(signer,w) [6] creator_kass_ata(w)
///  [7] contribution(pda,w) [8] token program [9] system program`.
#[allow(clippy::too_many_arguments)]
pub fn create_market(
    creator: &Pubkey,
    oracle: &Pubkey,
    kass_mint: &Pubkey,
    creator_kass_ata: &Pubkey,
    seed_amount: u64,
    outcome_index: u8,
) -> Instruction {
    let (config, _) = crate::pda::config();
    let (market, _) = crate::pda::market(oracle, outcome_index);
    let (escrow, _) = crate::pda::escrow(&market);
    let (contribution, _) = crate::pda::contribution(&market, creator);
    let mut data = vec![IX_CREATE_MARKET];
    data.extend_from_slice(&seed_amount.to_le_bytes());
    data.push(outcome_index);
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(config, false),
            AccountMeta::new_readonly(*oracle, false),
            AccountMeta::new(market, false),
            AccountMeta::new(escrow, false),
            AccountMeta::new_readonly(*kass_mint, false),
            AccountMeta::new(*creator, true),
            AccountMeta::new(*creator_kass_ata, false),
            AccountMeta::new(contribution, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    }
}

/// `Contribute` (Ix 3) — add `amount` KASS to a `Funding` market's escrow and
/// create-or-increment the contributor's `Contribution`.
/// Payload = `amount` (u64 LE). Accounts:
/// `[0] market(w) [1] escrow(w) [2] contributor(signer,w) [3] contributor_kass_ata(w)
///  [4] contribution(pda,w) [5] token program [6] system program`.
///
/// The processor reads only the first six accounts (its slice pattern tolerates
/// trailing accounts). The system program is appended so it is loaded into the
/// transaction: the first-ever contribution from a given contributor creates
/// their `Contribution` PDA via a CPI to the system program's `CreateAccount`,
/// which requires that program to be present in the tx account set (same as
/// `create_market`, which passes it explicitly).
pub fn contribute(
    contributor: &Pubkey,
    market: &Pubkey,
    escrow: &Pubkey,
    contributor_ata: &Pubkey,
    amount: u64,
) -> Instruction {
    let (contribution, _) = crate::pda::contribution(market, contributor);
    let mut data = vec![IX_CONTRIBUTE];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*market, false),
            AccountMeta::new(*escrow, false),
            AccountMeta::new(*contributor, true),
            AccountMeta::new(*contributor_ata, false),
            AccountMeta::new(contribution, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data,
    }
}

/// `Cancel` (Ix 4) — mark an under-funded `Funding` market `Cancelled` once its
/// underlying Kassandra oracle is terminal. Permissionless (no required signer
/// beyond the tx fee payer). Payload = empty. Accounts:
/// `[0] market(w) [1] oracle(ro)`.
pub fn cancel(market: &Pubkey, oracle: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*market, false),
            AccountMeta::new_readonly(*oracle, false),
        ],
        data: vec![IX_CANCEL],
    }
}

/// `Refund` (Ix 5) — permissionless per-contributor refund from a `Cancelled`
/// market. Program-signed transfer of the recorded stake out of escrow back to
/// the contributor's KASS ata, then the `Contribution` is CLOSED with its rent
/// returned to `contributor`. Payload = empty. Accounts:
/// `[0] market(w) [1] escrow(w) [2] contribution(w) [3] contributor_kass_ata(w)
///  [4] contributor(w) [5] token program`.
///
/// `market` is writable (its `open_contributions` counter is decremented) and
/// `contributor` (== `contribution.contributor`) receives the closed Contribution's
/// rent.
pub fn refund(
    market: &Pubkey,
    escrow: &Pubkey,
    contribution: &Pubkey,
    contributor_ata: &Pubkey,
    contributor: &Pubkey,
) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*market, false),
            AccountMeta::new(*escrow, false),
            AccountMeta::new(*contribution, false),
            AccountMeta::new(*contributor_ata, false),
            AccountMeta::new(*contributor, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: vec![IX_REFUND],
    }
}
