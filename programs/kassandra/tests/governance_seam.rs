//! Task F6 — v0.6 futarchy / Squads-v4 governance EXECUTION SEAM (Kassandra side).
//!
//! F6 was NARROWED to the documented seam fallback (see the plan's "Governance
//! seam" section + the F6 delta): driving the full v0.6 proposal → conditional
//! pass/fail market (v0.6 vault + Meteora DAMM v2) → trade → finalize → Squads
//! vault execute lifecycle inside LiteSVM is intractable. Instead we prove the
//! seam rigorously and document the rest as deferred.
//!
//! ## What this file proves (Kassandra side of the seam)
//! The DAO execution authority is a **Squads v4 multisig vault PDA** (F0 finding
//! #1). Kassandra records that vault PDA as `Protocol.dao_authority` and gates
//! `set_config` / `resolve_deadend` on it via `assert_dao_authority`. Here we:
//!
//! 1. DERIVE the Squads v4 vault PDA from the DOCUMENTED seeds
//!    (`[b"multisig", b"multisig", dao]` → multisig, then
//!    `[b"multisig", multisig, b"vault", [0]]` → vault, under `SQDS4…`),
//! 2. record THAT derived vault PDA as `Protocol.dao_authority` via
//!    `set_governance`, and
//! 3. show every privileged instruction (`set_config`, `resolve_deadend`)
//!    REJECTS a different signer with `Unauthorized`.
//!
//! ## Honest note on signing as the vault PDA
//! A LiteSVM test (like any client) CANNOT fabricate a signature for a PDA — only
//! the owning program can `invoke_signed` it. So once `dao_authority` is a Squads
//! vault PDA, NO test keypair can satisfy the gate; the accept-path is exercised
//! in F3/F4 with an ordinary recorded keypair (and re-anchored minimally below).
//! In PRODUCTION the vault-PDA signature is produced by Squads'
//! `vault_transaction_execute` CPI (whose discriminator + dispatch is validated
//! against the real `squads_v4.so` in
//! `metadao_v06_cpi::squads_vault_transaction_execute_discriminator_recognized`).
//! The composition — futarchy proposal passes → Squads executes the staged
//! `set_config`/`resolve_deadend` CPI signed by the vault PDA → Kassandra's gate
//! accepts because that PDA == `Protocol.dao_authority` — is the deferred
//! full-integration (a real-validator / surfpool follow-up; see the F6 delta).

mod common;
use common::*;

use kassandra_program::cpi::metadao_v06 as md6;
use kassandra_program::error::KassandraError;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};

/// Decode a LiteSVM transaction error into its `Custom(u32)` code, if any.
fn custom_code(res: &litesvm::types::TransactionResult) -> Option<u32> {
    use solana_sdk::instruction::InstructionError;
    use solana_sdk::transaction::TransactionError;
    match res {
        Err(meta) => match &meta.err {
            TransactionError::InstructionError(_, InstructionError::Custom(code)) => Some(*code),
            _ => None,
        },
        Ok(_) => None,
    }
}

/// Derive the Squads v4 multisig **vault** PDA (the DAO execution authority) for
/// a given DAO pubkey, using the documented seed builders in
/// [`kassandra_program::cpi::metadao_v06`] and the real Squads v4 program id.
/// `dao` is the futarchy `Dao` PDA (the multisig's `create_key`); the futarchy
/// DAO uses vault index 0.
fn derive_squads_vault(dao: &Pubkey) -> Pubkey {
    let squads_id = Pubkey::new_from_array(md6::SQUADS_V4_ID);
    let dao_arr = dao.to_bytes();
    let (multisig, _) =
        Pubkey::find_program_address(&md6::squads_multisig_seeds(&dao_arr), &squads_id);
    let multisig_arr = multisig.to_bytes();
    let (vault, _) =
        Pubkey::find_program_address(&md6::squads_vault_seeds(&multisig_arr, &[0u8]), &squads_id);
    vault
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. The realistic authority: a DERIVED Squads v4 vault PDA recorded as
//    `dao_authority`. `set_config` then rejects EVERY signer a test can produce
//    (the admin/payer and an unrelated keypair), with `Unauthorized` — proving
//    the gate accepts ONLY the recorded vault PDA, which in production only
//    Squads' vault_transaction_execute can sign for.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn set_config_gate_accepts_only_recorded_squads_vault_pda() {
    let mut ctx = TestCtx::new();
    ctx.ensure_protocol();

    // The futarchy Dao PDA stands in as the multisig's create_key; for the seam
    // any distinct pubkey works (the derivation, not the Dao's identity, is what
    // we exercise). Derive the real Squads vault PDA from the documented seeds.
    let dao = Pubkey::new_unique();
    let vault_pda = derive_squads_vault(&dao);
    let kass_dao = Pubkey::new_unique();

    // Admin (payer) hands governance off, recording the vault PDA as the DAO
    // execution authority.
    let payer = ctx.payer.insecure_clone();
    let (protocol_pda, res) = ctx.set_governance(&payer, vault_pda, kass_dao);
    assert!(res.is_ok(), "handoff should succeed: {res:?}");
    assert_eq!(
        ctx.protocol(protocol_pda).dao_authority,
        vault_pda.to_bytes(),
        "dao_authority must be the derived Squads vault PDA"
    );

    let params = ConfigParams::defaults();

    // (a) The admin/payer is NOT the vault PDA → Unauthorized.
    let (_pda, res) = ctx.set_config(&payer, params);
    assert_eq!(
        custom_code(&res),
        Some(KassandraError::Unauthorized as u32),
        "admin must not be able to set_config once governance is the vault PDA: {res:?}"
    );

    // (b) An unrelated funded keypair is NOT the vault PDA → Unauthorized.
    let stranger = Keypair::new();
    ctx.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let (_pda, res) = ctx.set_config(&stranger, params);
    assert_eq!(
        custom_code(&res),
        Some(KassandraError::Unauthorized as u32),
        "a stranger must not be able to set_config: {res:?}"
    );

    // The config is unchanged (no signer could pass the gate).
    let p = ctx.protocol(protocol_pda);
    assert_eq!(p.dao_authority, vault_pda.to_bytes());
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Same seam for `resolve_deadend`: with the derived vault PDA recorded as
//    `dao_authority`, a dead-ended oracle cannot be resolved by any test signer.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn resolve_deadend_gate_rejects_non_vault_signer() {
    let mut ctx = TestCtx::new();
    ctx.ensure_protocol();

    let dao = Pubkey::new_unique();
    let vault_pda = derive_squads_vault(&dao);
    let kass_dao = Pubkey::new_unique();

    let payer = ctx.payer.insecure_clone();
    let (_p, res) = ctx.set_governance(&payer, vault_pda, kass_dao);
    assert!(res.is_ok(), "handoff should succeed: {res:?}");

    // Stand up a dead-ended oracle.
    let oracle = ctx.seed_disputed_oracle(&[
        ProposerSpec {
            option: 0,
            bond: 1_000,
        },
        ProposerSpec {
            option: 1,
            bond: 1_000,
        },
    ]);
    ctx.set_phase(oracle, kassandra_program::state::Phase::InvalidDeadend);

    // A stranger (and the admin) cannot resolve it — only the recorded vault PDA.
    let stranger = Keypair::new();
    ctx.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let (_p, res) = ctx.resolve_deadend(oracle, &stranger, 0);
    assert_eq!(
        custom_code(&res),
        Some(KassandraError::Unauthorized as u32),
        "a stranger must not resolve a dead-end once governance is the vault PDA: {res:?}"
    );
    let (_p, res) = ctx.resolve_deadend(oracle, &payer, 0);
    assert_eq!(
        custom_code(&res),
        Some(KassandraError::Unauthorized as u32),
        "admin must not resolve a dead-end once governance is the vault PDA: {res:?}"
    );

    // The oracle is untouched: still dead-ended, not resolved.
    assert_eq!(
        ctx.oracle(oracle).phase,
        kassandra_program::state::Phase::InvalidDeadend.as_u8(),
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Accept-path anchor (minimal — fully covered in F3/F4): when the RECORDED
//    `dao_authority` is an ordinary, signable keypair, the very same gate ACCEPTS
//    it. This proves the gate is an identity check (accept iff signer == recorded
//    authority), not a blanket reject — so production's vault-PDA signature
//    (produced by Squads' vault_transaction_execute) will likewise be accepted.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn gate_accepts_recorded_authority_when_signable() {
    let mut ctx = TestCtx::new();
    ctx.ensure_protocol();

    let dao_kp = Keypair::new();
    ctx.svm.airdrop(&dao_kp.pubkey(), 1_000_000_000).unwrap();
    let kass_dao = Pubkey::new_unique();

    let payer = ctx.payer.insecure_clone();
    let (protocol_pda, res) = ctx.set_governance(&payer, dao_kp.pubkey(), kass_dao);
    assert!(res.is_ok(), "handoff should succeed: {res:?}");

    // The recorded authority signs → accepted.
    let mut params = ConfigParams::defaults();
    params.phase_window = 7200;
    let (_pda, res) = ctx.set_config(&dao_kp, params);
    assert!(
        res.is_ok(),
        "recorded authority set_config should succeed: {res:?}"
    );
    assert_eq!(ctx.protocol(protocol_pda).phase_window, 7200);
}
