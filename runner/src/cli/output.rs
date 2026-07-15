//! Output types serialized to stdout by `run` / `verify`.

use serde::Serialize;

use crate::constants::SUBMIT_AI_CLAIM_PAYLOAD_LEN;

/// The AiClaim PDA seed hint (echoed only when oracle/proposer are in the
/// config). The PDA is `find_program_address([b"claim", oracle, proposer],
/// program_id)`; we surface the seeds rather than derive the address (which
/// would also need the program id + base58 pubkey decoding).
#[derive(Clone, Debug, Serialize)]
pub struct ClaimPdaSeeds {
    /// The literal seed prefix (`b"claim"`).
    pub seed_prefix: String,
    /// The oracle pubkey seed.
    pub oracle: String,
    /// The proposer pubkey seed.
    pub proposer: String,
}

/// The `run` output (serialized to stdout as JSON).
#[derive(Clone, Debug, Serialize)]
pub struct RunOutput {
    /// The chosen categorical option index.
    pub option_index: u8,
    /// `sha256(model_id_string)` as hex.
    pub model_id_hex: String,
    /// `params_hash` as hex.
    pub params_hash_hex: String,
    /// `io_hash` as hex.
    pub io_hash_hex: String,
    /// The exact 97-byte `submit_ai_claim` payload (`model_id ++ params_hash ++
    /// io_hash ++ option`) as hex.
    pub submit_ai_claim_payload_hex: String,
    /// The resolved model identifier string actually recorded.
    pub resolved_model_id: String,
    /// The AiClaim PDA seeds, if oracle/proposer were provided.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_pda_seeds: Option<ClaimPdaSeeds>,
    /// The result of the on-chain submission, present only in `--submit` keeper
    /// mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission: Option<SubmissionOutput>,
    /// The exact 97-byte `submit_ai_claim` payload as raw bytes — the SAME bytes
    /// as [`Self::submit_ai_claim_payload_hex`]. Carried (not re-serialized) so
    /// the `--submit` path signs the runner's OWN payload verbatim rather than
    /// recomputing it.
    #[serde(skip)]
    pub submit_ai_claim_payload: [u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN],
}

/// The on-chain submission result appended to a `--submit` run.
#[derive(Clone, Debug, Serialize)]
pub struct SubmissionOutput {
    /// The confirmed transaction signature (base58).
    pub signature: String,
    /// The reached confirmation status (`confirmed` / `finalized`).
    pub confirmation_status: String,
    /// The oracle the claim was submitted against (base58).
    pub oracle: String,
    /// The derived Proposer PDA (`[b"proposer", oracle, authority]`, base58).
    pub proposer: String,
    /// The signing authority = the `--keypair` pubkey (base58).
    pub authority: String,
}

/// The result of comparing one submitted hash field.
#[derive(Clone, Debug, Serialize)]
pub struct HashCheck {
    /// What was submitted (hex).
    pub submitted: String,
    /// What we produced (hex).
    pub produced: String,
    /// Whether they match.
    pub matches: bool,
}

/// The `verify` output.
#[derive(Clone, Debug, Serialize)]
pub struct VerifyOutput {
    /// The full re-run output (produced option + hashes + payload).
    pub produced: RunOutput,
    /// The submitted claim's option.
    pub submitted_option: u8,
    /// Whether the produced option matches the submitted one.
    pub option_matches: bool,
    /// Human-readable advice.
    pub advice: String,
    /// Optional per-hash comparisons (only when submitted hashes were provided).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id_check: Option<HashCheck>,
    /// See [`Self::model_id_check`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params_hash_check: Option<HashCheck>,
    /// See [`Self::model_id_check`]. NOTE: against the live Anthropic provider an
    /// `io_hash` mismatch is EXPECTED and not grounds to challenge — the model's
    /// raw response text varies run-to-run, so `io_hash` (a commitment to the exact
    /// (input, output) the submitter used) rarely reproduces. Base the
    /// challenge/no-challenge decision on `option_matches`, not this field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io_hash_check: Option<HashCheck>,
}
