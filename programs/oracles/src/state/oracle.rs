//! The top-level dispute [`Oracle`] account.

use bytemuck::{Pod, Zeroable};
use pinocchio::cpi::Seed;

use super::{Phase, Pubkey};

/// Top-level dispute account. `size_of == 368` (was 360 before the `min_stake`
/// bootstrapping field; the earlier `392` predated the `prompt_hash` removal).
///
/// # Governable params snapshot (Task F2)
/// The behavioral governable params (`threshold_*`, `market_threshold_*`,
/// `flip_slash_*`, `phase_window`, `proposal_window`, plus the settlement-era
/// reserved `fact_vote_slash_*` / reward weights) are SNAPSHOTTED from the
/// [`Protocol`] at `create_oracle` and read by the downstream processors from
/// the `Oracle` they already load. New oracles pick up the current `Protocol`
/// config; in-flight oracles keep their snapshot, so a mid-dispute governance
/// change can never move the goalposts. F2 defaults them (via `init_protocol`)
/// to the current `config.rs` consts, so behavior is unchanged.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Oracle {
    pub account_type: u8, // AccountType::Oracle
    pub _pad_hdr: [u8; 7],
    pub creator: Pubkey,
    pub kass_mint: Pubkey,
    pub usdc_mint: Pubkey,
    pub stake_vault: Pubkey, // PDA token account holding all KASS bonds/stakes
    pub deadline: i64,       // unix; proposals rejected before this
    pub phase_ends_at: i64,  // end of the current window
    pub twap_window: i64,    // per-oracle, seconds
    pub options_count: u8,   // number of categorical options
    pub phase: u8,           // Phase as u8
    pub proposer_count: u16,
    pub surviving_count: u16, // proposers not disqualified
    pub fact_count: u16,
    // Conservation accumulator; equals the `stake_vault` balance UNTIL a
    // challenge splits a proposer's bond into a MetaDAO conditional vault —
    // Task 13 conservation must also count conditional-vault-held KASS recorded
    // on the corresponding `Market` (`open_challenge` does NOT decrement this).
    pub total_oracle_stake: u64,
    pub bond_pool: u64,          // accumulated slashed KASS (base units)
    pub dispute_bond_total: u64, // Σ proposer bonds, fixed at dispute start; fact-quorum denominator
    pub settled_count: u16,      // facts settled so far (drives incremental finalize)
    pub ai_finalized_count: u16, // proposers ai-finalized so far (drives incremental finalize_ai_claims)
    pub bump: u8,
    // Final resolved categorical option, written by `finalize_oracle`. CONTRACT:
    // it is the winning option ONLY when `phase == Resolved`. On the terminal
    // [`Phase::InvalidDeadend`] (tie / no survivors) finalize_oracle stamps it
    // with the loud `CLAIM_OPTION_NONE` (0xFF) sentinel, so a consumer that
    // forgets to gate on `phase == Resolved` reads `0xFF` rather than a plausible
    // "option 0 won." Before finalize (any non-terminal phase) it is its zeroed
    // default and must not be read. (Originally absorbed the former `_pad1[1]`;
    // Oracle has since grown — see the struct docstring for the current LEN.)
    pub resolved_option: u8,
    // Number of OPEN (created-but-not-yet-settled) challenge decision markets.
    // `open_challenge` does `checked_add(1)` when it creates a Market;
    // `settle_challenge` does `checked_sub(1)` when it sets `market.settled`.
    // Task 12's `finalize_oracle` REQUIRES this == 0 before recomputing the
    // final plurality, so an unsettled challenged proposer can never be wrongly
    // counted as surviving. (Originally fit the former `_pad1`; Oracle has since
    // grown — see the struct docstring for the current LEN.)
    pub open_challenge_count: u16,
    // NOTE: the former `prompt_hash` [u8;32] lived here. It was write-only (never
    // read on-chain); the plaintext subject now lives on-chain in the companion
    // `[b"oracle_meta", oracle]` account, so the hash was removed. `threshold_num`
    // (8-aligned) follows `open_challenge_count` directly — the struct shrank by
    // 32 bytes with no padding (Pod derive enforces this).
    // ---- Governable params snapshotted from `Protocol` at create_oracle (F2) -
    // Read by the downstream processors instead of the `config.rs` consts; equal
    // to the consts by default so behavior is unchanged.
    pub threshold_num: u64, // fact-quorum supermajority (finalize_facts)
    pub threshold_den: u64, // fact-quorum supermajority (finalize_facts)
    pub market_threshold_num: u64, // slash-trigger margin (settle_challenge; widened to u128 on use)
    pub market_threshold_den: u64, // slash-trigger margin (settle_challenge; widened to u128 on use)
    pub flip_slash_num: u64,       // flip-slash fraction (finalize_ai_claims)
    pub flip_slash_den: u64,       // flip-slash fraction (finalize_ai_claims)
    pub phase_window: i64,         // dispute phase window seconds
    pub proposal_window: i64,      // proposal-registration window seconds
    // ---- Reserved (settlement-era; snapshotted but no on-chain reader yet) ---
    pub fact_vote_slash_num: u64,
    pub fact_vote_slash_den: u64,
    pub reward_proposer_weight: u64,
    pub reward_fact_weight: u64,
    // ---- Challenge-fee config snapshot (Task C1) -----------------------------
    // Directional challenge-market fees, snapshotted from `Protocol` at
    // create_oracle (so an in-flight market keeps its rates if governance
    // retunes). USDC fee on a FAILED challenge (→ proposer) and KASS fee on a
    // SUCCESSFUL challenge (→ challenger); consumed by settle (Task C2).
    pub challenge_fail_usdc_fee_num: u64,
    pub challenge_fail_usdc_fee_den: u64,
    pub challenge_success_kass_fee_num: u64,
    pub challenge_success_kass_fee_den: u64,
    // ---- Settlement resolution totals (Task S1) ------------------------------
    // Stamped at resolution for the per-staker S2 pull-claims to read; all 0
    // until then (and 0 at create). NO token movement is done in S1 — these are
    // pure accumulators/stamps the later claim instructions consume.
    //
    // `total_correct_proposer_stake`: Σ `bond` over SURVIVING proposers whose
    //   `claim_option == resolved_option`. Stamped by `finalize_oracle` on the
    //   Resolved branch (the pro-rata denominator for the proposer reward bucket).
    // `total_approved_fact_stake`: Σ (`fact.stake` + `fact.approve_stake`) over
    //   AGREED facts (submitter stake + approve-voter stake that earns the
    //   fact_rate). Accumulated incrementally by `finalize_facts` as facts settle.
    // `reward_pool`: the distributable reward pool finalized at resolution. On
    //   Resolved it is set to `bond_pool` (S3 will fold `reward_emission` in here:
    //   `reward_pool = bond_pool + reward_emission`). Left 0 on InvalidDeadend.
    pub total_correct_proposer_stake: u64,
    pub total_approved_fact_stake: u64,
    pub reward_pool: u64,
    // ---- Emission minted at creation (Task S3) -------------------------------
    // KASS minted into `stake_vault` by `create_oracle` from the supply reservoir
    // (`reward_emission = (total_supply_cap − kass_supply) · emission_num/den`,
    // computed AFTER the EMA fee burn so the burn boosts the same-tx reservoir),
    // recorded here. On the `Resolved` branch `finalize_oracle` folds it into
    // `reward_pool` (`reward_pool = bond_pool + reward_emission`); on
    // `InvalidDeadend` it is BURNED back from `stake_vault` to the reservoir so a
    // dead-end leaks no emission. 0 when emission is disabled (`total_supply_cap
    // == 0` or `emission_num == 0`) — the genesis/disabled default.
    pub reward_emission: u64,
    // ---- Activity-scaled stake floor (bootstrapping) -------------------------
    // The minimum stake (KASS base units) required by `propose` / `submit_fact` /
    // `vote_fact` on this oracle, snapshotted at `create_oracle` from the decayed
    // fee-EMA via `crate::stake_floor::stake_floor`. 0 at genesis / low activity
    // (free participation, no premined KASS) and while the magnitude is disabled
    // (`Protocol.stake_floor_max == 0`). Frozen for the oracle's whole life so a
    // later governance retune never moves an in-flight oracle's floor.
    pub min_stake: u64,
}

impl Oracle {
    pub const LEN: usize = core::mem::size_of::<Oracle>();

    /// The oracle PDA seed prefix: the account lives at `[SEED_PREFIX, nonce_le]`.
    pub const SEED_PREFIX: &'static [u8] = b"oracle";

    /// Decode the stored phase discriminant.
    pub fn phase(&self) -> Option<Phase> {
        Phase::from_u8(self.phase)
    }

    /// Write the phase discriminant.
    pub fn set_phase(&mut self, p: Phase) {
        self.phase = p as u8;
    }

    /// The oracle PDA's program-signer seeds `[b"oracle", nonce_le, [bump]]` — the
    /// single source of truth every processor uses to sign token moves out of the
    /// oracle's vaults. The caller owns the `nonce_le` + `bump` buffers (they must
    /// outlive the returned `Seed`s).
    pub fn signer_seeds<'a>(nonce_le: &'a [u8; 8], bump: &'a [u8; 1]) -> [Seed<'a>; 3] {
        [
            Seed::from(Self::SEED_PREFIX),
            Seed::from(nonce_le.as_ref()),
            Seed::from(bump.as_ref()),
        ]
    }
}
