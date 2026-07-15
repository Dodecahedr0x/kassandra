//! The [`Protocol`] singleton: the program's global configuration record.

use bytemuck::{Pod, Zeroable};

use super::Pubkey;

/// Protocol singleton: the program's global configuration record. `size_of == 368`.
///
/// Created once by `init_protocol` and never re-initialized. Pins the canonical
/// KASS/USDC mints (so `create_oracle`'s fee-burn cannot be spoofed with a fake
/// KASS mint) and carries the dynamic creation-fee EMA state used by Task H2.
///
/// # Governance linkage (Task F1)
/// `dao_authority` is the **Squads v4 multisig VAULT PDA** that gates the
/// privileged `set_config`/`resolve_deadend` instructions; `kass_dao` is the
/// futarchy `Dao` account whose embedded spot AMM is the KASS price source
/// (F5). Both are zero (unset) at `init_protocol` and recorded once by
/// `set_governance` (the one-time admin→DAO handoff). `governance_set` is the
/// one-shot flag (see `set_governance` for the trust model).
///
/// # Governable monetary params (Task F1)
/// The global monetary knobs (`emission_*`, `total_supply_cap`, and the fee-EMA
/// params) live here so `set_config` (F3) can retune them and `create_oracle`
/// can read them from state. F1 only ADDS them and defaults them to the current
/// `config.rs` consts so behavior is unchanged; the config-as-state migration
/// (wiring `create_oracle` to read these instead of the consts) is F2.
///
/// # Protocol PDA seeds (CONTRACT)
/// `[b"protocol"]` (singleton), program = [`crate::ID`].
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Protocol {
    pub account_type: u8, // AccountType::Protocol
    pub _pad_hdr: [u8; 7],
    pub admin: Pubkey,     // the initializer; gates the one-time set_governance
    pub kass_mint: Pubkey, // canonical KASS mint; oracles must match this
    pub usdc_mint: Pubkey, // canonical USDC mint; oracles must match this
    // Fixed-point EMA accumulator of recent oracle-creation activity. 0 at
    // genesis (first creation is free); rises with creation frequency and decays
    // when idle. Drives the dynamic creation fee in Task H2. Unused (always 0)
    // until then.
    pub fee_ema: u64,
    // Unix timestamp of the most recent oracle creation, for the EMA decay in
    // Task H2. 0 at genesis.
    pub last_creation_unix: i64,
    pub bump: u8,
    // 1 once `set_governance` has recorded `dao_authority`/`kass_dao`; 0 before
    // (the admin→DAO handoff is one-shot, see `set_governance`).
    pub governance_set: u8,
    pub _pad: [u8; 6],
    // Squads v4 multisig VAULT PDA — the signer that gates `set_config` (F3) and
    // `resolve_deadend` (F4). Zero until `set_governance` records it.
    pub dao_authority: Pubkey,
    // Futarchy `Dao` account; its embedded spot AMM is F5's KASS price source.
    // Zero until `set_governance` records it. STORED (not re-derived) because the
    // `Dao` account's post-`amm` fields sit at variable offsets (F0 finding).
    pub kass_dao: Pubkey,
    // ---- Governable monetary params (reserved by F1, retuned by F3) ----------
    // Emission rate as a fraction `emission_num / emission_den`. Settlement sets
    // the full semantics; F1 reserves the fields (defaulted 0/1 — no emission,
    // denominator never zero) so the layout and `set_config` plumbing exist now.
    pub emission_num: u64,
    pub emission_den: u64,
    // Hard cap on circulating KASS supply (settlement-era; F1 reserves it as 0).
    pub total_supply_cap: u64,
    // Mirror of the `config.rs` fee-EMA consts so `create_oracle` can later read
    // them from state (F2). F1 defaults them to the current consts (no behavior
    // change): `FEE_EMA_HALFLIFE_SECS`, `FEE_PER_EMA_UNIT`, `FEE_EMA_INCREMENT`.
    pub fee_ema_halflife: i64,
    pub fee_per_ema_unit: u64,
    pub fee_ema_increment: u64,
    // ---- Governable behavioral params (F2 — mutable source, set_config edits) -
    // Snapshotted onto each `Oracle` at `create_oracle`. `init_protocol` defaults
    // them to the current `config.rs` consts so behavior is unchanged. The
    // active ones are read by the downstream processors via the per-oracle
    // snapshot, never from `Protocol` directly.
    pub threshold_num: u64,        // fact-quorum supermajority (THRESHOLD_NUM)
    pub threshold_den: u64,        // fact-quorum supermajority (THRESHOLD_DEN)
    pub market_threshold_num: u64, // slash-trigger margin (MARKET_THRESHOLD_NUM; u128 on use)
    pub market_threshold_den: u64, // slash-trigger margin (MARKET_THRESHOLD_DEN; u128 on use)
    pub flip_slash_num: u64,       // flip-slash fraction (FLIP_SLASH_NUM)
    pub flip_slash_den: u64,       // flip-slash fraction (FLIP_SLASH_DEN)
    pub phase_window: i64,         // dispute phase window seconds (PHASE_WINDOW)
    pub proposal_window: i64,      // proposal-registration window seconds (PROPOSAL_WINDOW)
    // ---- Reserved (settlement-era; defaulted, no reader yet) -----------------
    pub fact_vote_slash_num: u64,
    pub fact_vote_slash_den: u64,
    pub reward_proposer_weight: u64,
    pub reward_fact_weight: u64,
    // ---- Challenge-fee config (Task C1; mutable source, snapshotted to Oracle)
    // USDC fee on a FAILED challenge (→ proposer) and KASS fee on a SUCCESSFUL
    // challenge (→ challenger), each a `num/den` fraction. Defaulted by
    // `init_protocol` (1/100 each), retuned by `set_config` (den>0, num≤den).
    pub challenge_fail_usdc_fee_num: u64,
    pub challenge_fail_usdc_fee_den: u64,
    pub challenge_success_kass_fee_num: u64,
    pub challenge_success_kass_fee_den: u64,
    // ---- Activity-scaled stake-floor curve (bootstrapping; snapshotted to Oracle)
    // The governable curve `create_oracle` evaluates against the decayed fee-EMA to
    // snapshot `Oracle.min_stake` (see `crate::stake_floor`). `init_protocol`
    // defaults threshold/cap to the recommended shape (`STAKE_FLOOR_EMA_*`) and the
    // magnitude `stake_floor_max` to 0 = disabled (participation always free) until
    // governance activates it via `set_config`.
    pub stake_floor_ema_threshold: u64, // fee-EMA below which the floor is 0
    pub stake_floor_ema_cap: u64,       // fee-EMA at which the floor reaches max
    pub stake_floor_max: u64,           // max floor (KASS base units); 0 = disabled
}

impl Protocol {
    pub const LEN: usize = core::mem::size_of::<Protocol>();

    /// Whether `set_governance` has recorded the DAO linkage.
    pub fn is_governance_set(&self) -> bool {
        self.governance_set != 0
    }
}
