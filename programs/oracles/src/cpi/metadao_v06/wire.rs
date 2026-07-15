//! Program IDs, Anchor instruction/account discriminators, and PDA seed prefixes.

use pinocchio::address::Address as Pubkey;

// ─────────────────────────────────────────────────────────────────────────────
// Program IDs
// ─────────────────────────────────────────────────────────────────────────────

/// MetaDAO `futarchy` v0.6.0 governance/proposal program (mainnet-beta). Replaces
/// the legacy `autocrat`.
pub const FUTARCHY_ID: Pubkey =
    Pubkey::from_str_const("FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq");

/// MetaDAO `conditional_vault` (v0.6 line). Byte-for-byte the same deployed
/// program as the v0.4 vault ([`super::super::metadao::CONDITIONAL_VAULT_ID`]); v0.6
/// reuses it. Its instruction/account discriminators are unchanged.
pub const CONDITIONAL_VAULT_V06_ID: Pubkey =
    Pubkey::from_str_const("VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg");

/// Meteora DAMM v2 (cp-amm) program (mainnet-beta).
pub const METEORA_DAMM_V2_ID: Pubkey =
    Pubkey::from_str_const("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");

/// Squads v4 multisig program — hosts the DAO execution-authority vault PDA.
pub const SQUADS_V4_ID: Pubkey =
    Pubkey::from_str_const("SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf");

// ─────────────────────────────────────────────────────────────────────────────
// Anchor instruction discriminators — sha256("global:<name>")[..8]
// ─────────────────────────────────────────────────────────────────────────────
//
// futarchy v0.6 (programs/futarchy/src/lib.rs @ v0.6.0).

/// `futarchy::initialize_dao` — args `InitializeDaoParams` (Borsh; see
/// [`InitializeDaoParams`] doc for the field order).
pub const FUT_INITIALIZE_DAO: [u8; 8] = [0x80, 0xe2, 0x60, 0x5a, 0x27, 0x38, 0x18, 0xc4];
/// `futarchy::initialize_proposal` — no positional args (accounts only).
pub const FUT_INITIALIZE_PROPOSAL: [u8; 8] = [0x32, 0x49, 0x9c, 0x62, 0x81, 0x95, 0x15, 0x9e];
/// `futarchy::launch_proposal`.
pub const FUT_LAUNCH_PROPOSAL: [u8; 8] = [0x10, 0xd3, 0xbd, 0x77, 0xf5, 0x48, 0x00, 0xe5];
/// `futarchy::finalize_proposal` — resolves the conditional question + sets
/// `ProposalState::{Passed,Failed}` from the pass/fail TWAP comparison.
pub const FUT_FINALIZE_PROPOSAL: [u8; 8] = [0x17, 0x44, 0x33, 0xa7, 0x6d, 0xad, 0xbb, 0xa4];
/// `futarchy::update_dao` — args `UpdateDaoParams`.
pub const FUT_UPDATE_DAO: [u8; 8] = [0x83, 0x48, 0x4b, 0x19, 0x70, 0xd2, 0x6d, 0x02];
/// `futarchy::spot_swap` — swaps against the embedded spot AMM (cranks its TWAP).
pub const FUT_SPOT_SWAP: [u8; 8] = [0xa7, 0x61, 0x0c, 0xe7, 0xed, 0x4e, 0xa6, 0xfb];
/// `futarchy::conditional_swap` — swaps against a pass/fail conditional market.
pub const FUT_CONDITIONAL_SWAP: [u8; 8] = [0xc2, 0x88, 0xdc, 0x59, 0xf2, 0xa9, 0x82, 0x9d];

/// `Dao` account discriminator (`sha256("account:Dao")[..8]`).
pub const DAO_ACCOUNT_DISCRIMINATOR: [u8; 8] = [0xa3, 0x09, 0x2f, 0x1f, 0x34, 0x55, 0xc5, 0x31];
/// `Proposal` account discriminator (`sha256("account:Proposal")[..8]`).
pub const PROPOSAL_ACCOUNT_DISCRIMINATOR: [u8; 8] =
    [0x1a, 0x5e, 0xbd, 0xbb, 0x74, 0x88, 0x35, 0x21];

// Meteora DAMM v2 (cp-amm, MeteoraAg/damm-v2 @ main).

/// `cp_amm::initialize_pool`.
pub const METEORA_INITIALIZE_POOL: [u8; 8] = [0x5f, 0xb4, 0x0a, 0xac, 0x54, 0xae, 0xe8, 0x28];
/// `cp_amm::swap`.
pub const METEORA_SWAP: [u8; 8] = [0xf8, 0xc6, 0x9e, 0x91, 0xe1, 0x75, 0x87, 0xc8];
/// `cp_amm::add_liquidity`.
pub const METEORA_ADD_LIQUIDITY: [u8; 8] = [0xb5, 0x9d, 0x59, 0x43, 0x8f, 0xb6, 0x34, 0x48];
/// `Pool` account discriminator (`sha256("account:Pool")[..8]`).
pub const METEORA_POOL_ACCOUNT_DISCRIMINATOR: [u8; 8] =
    [0xf1, 0x9a, 0x6d, 0x04, 0x11, 0xb1, 0x6d, 0xbc];

// Squads v4 (Squads-Protocol/v4 @ rev 6d5235da). Squads v4 is an **Anchor**
// program, so its instruction selectors use the SAME scheme as futarchy/Meteora:
// `sha256("global:<snake_case_name>")[..8]`. (Confirmed against the dumped
// `squads_v4.so`: the [`SQUADS_VAULT_TRANSACTION_EXECUTE`] discriminator below
// dispatches into the program's `VaultTransactionExecute` handler — see the
// F6 dispatch-probe test in `tests/governance_seam.rs`.)
//
// The DAO-execution seam Kassandra cares about is `vault_transaction_execute`:
// a passed futarchy proposal's actions (a `set_config` / `resolve_deadend` CPI
// into Kassandra) are wrapped in a Squads `VaultTransaction` and run by this
// instruction, which `invoke_signed`s each inner instruction with the
// [`squads_vault_pda`] as the signing authority. That vault PDA is exactly what
// Kassandra stores as `Protocol.dao_authority`.

/// `squads_multisig_program::vault_transaction_execute` — runs a created vault
/// transaction, signing inner instructions as the multisig's vault PDA. This is
/// the instruction that produces the `dao_authority` (vault-PDA) signature on
/// Kassandra's `set_config` / `resolve_deadend` in production.
pub const SQUADS_VAULT_TRANSACTION_EXECUTE: [u8; 8] =
    [0xc2, 0x08, 0xa1, 0x57, 0x99, 0xa4, 0x19, 0xab];
/// `squads_multisig_program::vault_transaction_create` — stages the inner
/// instructions (the proposal's actions) into a `VaultTransaction` PDA.
pub const SQUADS_VAULT_TRANSACTION_CREATE: [u8; 8] =
    [0x30, 0xfa, 0x4e, 0xa8, 0xd0, 0xe2, 0xda, 0xd3];
/// `squads_multisig_program::proposal_create`.
pub const SQUADS_PROPOSAL_CREATE: [u8; 8] = [0xdc, 0x3c, 0x49, 0xe0, 0x1e, 0x6c, 0x4f, 0x9f];
/// `squads_multisig_program::multisig_create_v2` — the CPI `initialize_dao`
/// makes to stand up the DAO's multisig (`create_key` == the futarchy `Dao`).
pub const SQUADS_MULTISIG_CREATE_V2: [u8; 8] = [0x32, 0xdd, 0xc7, 0x5d, 0x28, 0xf5, 0x8b, 0xe9];

// ─────────────────────────────────────────────────────────────────────────────
// PDA seeds
// ─────────────────────────────────────────────────────────────────────────────

/// futarchy `Dao` PDA seed prefix.
pub const SEED_DAO: &[u8] = b"dao";
/// futarchy `Proposal` PDA seed prefix.
pub const SEED_PROPOSAL: &[u8] = b"proposal";
/// Anchor `#[event_cpi]` event-authority PDA seed (under the futarchy program).
pub const SEED_EVENT_AUTHORITY: &[u8] = b"__event_authority";

// Squads v4 seeds (Squads-Protocol/v4 @ rev 6d5235da, state/seeds.rs).
/// Squads `SEED_PREFIX`.
pub const SQUADS_SEED_PREFIX: &[u8] = b"multisig";
/// Squads `SEED_MULTISIG`.
pub const SQUADS_SEED_MULTISIG: &[u8] = b"multisig";
/// Squads `SEED_VAULT`.
pub const SQUADS_SEED_VAULT: &[u8] = b"vault";
