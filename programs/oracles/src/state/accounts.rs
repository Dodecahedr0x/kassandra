//! Per-dispute participant/record accounts: [`Proposer`], [`Fact`],
//! [`FactVote`], [`AiClaim`], and the challenge [`Market`] binding.

use bytemuck::{Pod, Zeroable};

use super::Pubkey;

/// A proposer's commitment within an oracle. `size_of == 96`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Proposer {
    pub account_type: u8, // AccountType::Proposer
    pub _pad_hdr: [u8; 7],
    pub oracle: Pubkey,
    pub authority: Pubkey,
    pub bond: u64,           // locked KASS
    pub original_option: u8, // value at proposal time (no proofs)
    // CONTRACT: `claim_option` MUST be initialized to `CLAIM_OPTION_NONE`
    // (0xFF) when a Proposer account is created — NOT left zeroed. A zeroed
    // value (0) would be misread as a valid claim for option 0, escaping the
    // no-show full-slash in `finalize_ai_claims` and counting as a real vote in
    // the Task 8 plurality. The proposer-registration / propose processor (not
    // yet built) must set it; the test harness already does.
    pub claim_option: u8, // value after AI claim; CLAIM_OPTION_NONE = not yet submitted
    pub disqualified: u8, // bool
    pub slashed: u8,      // bool
    pub flipped: u8,      // bool: claim_option != original_option
    pub bump: u8,
    pub ai_finalized: u8, // bool: settled by finalize_ai_claims (idempotency marker)
    pub _pad: [u8; 1],
    // KASS slashed from this proposer into the oracle's `bond_pool`. Set
    // authoritatively on EVERY slash path: `finalize_ai_claims` (no-show => bond;
    // flip => bond*FLIP_SLASH_NUM/FLIP_SLASH_DEN), `settle_challenge`
    // (challenge-fail => bond), and the `finalize_facts` no-facts dead-end
    // (=> bond). Invariant: a proposer's contribution to `bond_pool` always
    // equals its `slashed_amount`, so the deferred settlement layer (and Task 13
    // conservation) reconciles uniformly without a path-specific special case.
    pub slashed_amount: u64,
}

impl Proposer {
    pub const LEN: usize = core::mem::size_of::<Proposer>();

    pub fn is_disqualified(&self) -> bool {
        self.disqualified != 0
    }
    pub fn is_slashed(&self) -> bool {
        self.slashed != 0
    }
    pub fn is_flipped(&self) -> bool {
        self.flipped != 0
    }
    pub fn is_ai_finalized(&self) -> bool {
        self.ai_finalized != 0
    }
}

/// A fact submitted in support of an option. `size_of == 336`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Fact {
    pub account_type: u8, // AccountType::Fact
    pub _pad_hdr: [u8; 7],
    pub oracle: Pubkey,
    pub proposer: Pubkey, // who submitted the fact
    pub content_hash: [u8; 32],
    pub stake: u64,
    pub approve_stake: u64,   // running tally
    pub duplicate_stake: u64, // running tally of "duplicate" votes
    pub uri_len: u16,
    pub agreed: u8,    // set at finalize: 1 if accepted
    pub duplicate: u8, // set at finalize: 1 if duplicate-dominant
    pub settled: u8,   // bool
    pub bump: u8,
    pub _pad: [u8; 2],
    pub uri: [u8; 200],
}

impl Fact {
    pub const LEN: usize = core::mem::size_of::<Fact>();

    pub fn is_agreed(&self) -> bool {
        self.agreed != 0
    }
    pub fn is_duplicate(&self) -> bool {
        self.duplicate != 0
    }
    pub fn is_settled(&self) -> bool {
        self.settled != 0
    }
}

/// A stake-weighted vote on a fact. `size_of == 88`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct FactVote {
    pub account_type: u8, // AccountType::FactVote
    pub _pad_hdr: [u8; 7],
    pub fact: Pubkey,
    pub voter: Pubkey,
    pub stake: u64,
    pub kind: u8, // 0 = approve, 1 = duplicate
    pub bump: u8,
    pub _pad: [u8; 6],
}

impl FactVote {
    pub const LEN: usize = core::mem::size_of::<FactVote>();
}

/// A pinned-model AI claim for a proposer's option. `size_of == 208`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct AiClaim {
    pub account_type: u8, // AccountType::AiClaim
    pub _pad_hdr: [u8; 7],
    pub oracle: Pubkey,
    pub proposer: Pubkey,
    pub model_id: [u8; 32],    // hash/ident of pinned model
    pub params_hash: [u8; 32], // hash of declared params (temp, seed, ...)
    pub io_hash: [u8; 32],     // hash(prompt + agreed facts + raw response)
    pub option: u8,
    pub challenged: u8, // bool
    pub bump: u8,
    pub _pad: [u8; 5],
    // The proposer's HUMAN authority (== `proposer.authority`), stamped at submit
    // by `submit_ai_claim`. Recorded on the claim itself so the settlement-era
    // `close_ai_claim` (Task S4) routes the reclaimed rent to the authority
    // DIRECTLY — without loading the `Proposer` account, which `claim_proposer`
    // may have already closed. Makes the close ORDER-INDEPENDENT (rent never
    // stranded). Appended at offset 176 (clean ABI addition; all prior offsets
    // unchanged).
    pub authority: Pubkey,
}

impl AiClaim {
    pub const LEN: usize = core::mem::size_of::<AiClaim>();

    pub fn is_challenged(&self) -> bool {
        self.challenged != 0
    }
}

/// A challenge decision-market binding for one [`AiClaim`]. `size_of == 416`.
///
/// Created lazily by `open_challenge` only when a claim is actually challenged
/// — uncontested claims have NO `Market` account (markets are dormant by
/// default, design §6). It RECORDS the MetaDAO accounts the challenger composed
/// (a binary pass/fail `question` whose resolver is the Kassandra oracle PDA, a
/// KASS conditional vault, a USDC conditional vault, and the pass/fail AMMs),
/// the oracle-PDA-owned conditional-KASS destinations the proposer's bond was
/// split into, and the challenger's committed USDC — so `settle_challenge`
/// (Task 11) can read the TWAP, resolve the question, and redeem from the exact
/// recorded accounts (no off-chain bookkeeping). The security-critical bindings
/// (question.oracle, vault underlying mints, dest owner/mint) are verified at
/// creation; this struct is the durable record of that binding.
///
/// # Market PDA seeds (CONTRACT)
/// `[b"market", ai_claim_pubkey]`, program = [`crate::ID`].
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Market {
    pub account_type: u8, // AccountType::Market
    pub _pad_hdr: [u8; 7],
    pub oracle: Pubkey,
    pub ai_claim: Pubkey,
    pub proposer: Pubkey,
    pub challenger: Pubkey,
    pub question: Pubkey,   // MetaDAO binary question (resolver == oracle PDA)
    pub kass_vault: Pubkey, // MetaDAO conditional vault, underlying == oracle.kass_mint
    pub usdc_vault: Pubkey, // MetaDAO conditional vault, underlying == oracle.usdc_mint
    // DEFERRED-MUST-VERIFY-IN-TASK-11: only owner==AMM_ID was checked at
    // open_challenge; settle_challenge MUST verify each AMM is bound to this
    // market's pass/fail conditional (KASS,USDC) mint pair and that
    // pass_amm != fail_amm before reading its TWAP.
    pub pass_amm: Pubkey, // outcome-0 (pass) AMM
    pub fail_amm: Pubkey, // outcome-1 (fail) AMM
    // Oracle-PDA-owned conditional-KASS token accounts the proposer's bond was
    // split into (outcome 0 = pass, 1 = fail). Verified owner==oracle PDA and
    // mint==derived conditional KASS mint at creation; Task 11 redeems/settles
    // from exactly these.
    pub oracle_pass_kass: Pubkey,
    pub oracle_fail_kass: Pubkey,
    // Market-owned USDC escrow token account holding the challenger's staked
    // USDC (Task C1). SPL token account on `oracle.usdc_mint`, token authority =
    // the oracle PDA (mirrors `oracle.stake_vault`), at PDA
    // `[b"challenge_usdc", market]`. `open_challenge` creates + funds it;
    // settle (Task C2) returns it / carves the directional USDC fee.
    pub challenger_usdc_vault: Pubkey,
    pub twap_end: i64, // now + oracle.twap_window; settle allowed only after
    // Challenger's escrowed USDC (Task C1): computed on-chain at open_challenge
    // as `bond × kass_price` (raw USDC base units) and actually transferred into
    // `challenger_usdc_vault` — no longer an untrusted payload value.
    pub challenger_usdc: u64,
    pub settled: u8, // bool; set by settle_challenge (Task 11)
    pub bump: u8,
    pub _pad: [u8; 6],
}

impl Market {
    pub const LEN: usize = core::mem::size_of::<Market>();

    pub fn is_settled(&self) -> bool {
        self.settled != 0
    }
}
