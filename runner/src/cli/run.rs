//! The `run` / `verify` cores (mock-testable) + the `--submit` keeper pieces.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;

use crate::anthropic::{DEFAULT_MAX_TOKENS, DEFAULT_MODEL, PROVIDER_ID, THINKING_MODE};
use crate::constants::SUBMIT_AI_CLAIM_PAYLOAD_LEN;
use crate::fetch::{fetch_and_verify_facts, FactFetcher};
use crate::hashing::ClaimMetadata;
use crate::prompt::build_request;
use crate::provider::{AiProvider, ModelConfig};
use crate::submit::{derive_proposer_pda, submit_and_confirm, ConfirmOptions, SubmitError};

use crate::cli::{
    ClaimPdaSeeds, CommonArgs, HashCheck, RunOutput, RunnerConfig, SubmissionOutput, VerifyOutput,
};

// --- core (mock-testable: takes trait objects) ------------------------------

/// Build the [`ModelConfig`] from CLI knobs (defaults pinned to Opus 4.8 +
/// adaptive thinking). Centralizes the config so `params_hash` is stable.
pub fn build_model_config(model: Option<String>, max_tokens: Option<u32>) -> ModelConfig {
    ModelConfig {
        model_id: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        provider: PROVIDER_ID.to_string(),
        max_tokens: max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
        thinking: Some(THINKING_MODE.to_string()),
    }
}

/// The `run` core, generic over the fetcher + provider via trait objects so it
/// is fully testable offline with [`crate::fetch::MockFactFetcher`] +
/// [`MockProvider`]. `main` passes the real [`HttpFactFetcher`] +
/// [`AnthropicProvider`].
pub async fn run_core(
    config: &RunnerConfig,
    model_config: ModelConfig,
    fetcher: &dyn FactFetcher,
    provider: &dyn AiProvider,
) -> anyhow::Result<RunOutput> {
    let fact_refs = config.fact_refs()?;
    let facts = fetch_and_verify_facts(fetcher, &fact_refs).await?;

    let options = config.categorical_options();
    let req = build_request(&config.interpretation, &facts, options, model_config);
    let resp = provider.complete(&req).await?;

    let meta = ClaimMetadata::compute(&req, &resp);
    let payload = meta.to_payload(resp.option_index);

    let claim_pda_seeds = match (&config.oracle, &config.proposer) {
        (Some(oracle), Some(proposer)) => Some(ClaimPdaSeeds {
            seed_prefix: "claim".to_string(),
            oracle: oracle.clone(),
            proposer: proposer.clone(),
        }),
        _ => None,
    };

    Ok(RunOutput {
        option_index: resp.option_index,
        model_id_hex: hex::encode(&meta.model_id),
        params_hash_hex: hex::encode(&meta.params_hash),
        io_hash_hex: hex::encode(&meta.io_hash),
        submit_ai_claim_payload_hex: hex::encode(&payload),
        resolved_model_id: resp.model_id,
        claim_pda_seeds,
        submission: None,
        submit_ai_claim_payload: payload,
    })
}

/// The submitted claim a `verify` run compares against.
#[derive(Clone, Debug, Default)]
pub struct SubmittedClaim {
    /// The submitted categorical option.
    pub option: u8,
    /// Optional submitted `model_id` (hex) to compare.
    pub model_id_hex: Option<String>,
    /// Optional submitted `params_hash` (hex) to compare.
    pub params_hash_hex: Option<String>,
    /// Optional submitted `io_hash` (hex) to compare.
    pub io_hash_hex: Option<String>,
}

/// The `verify` core: re-run, then compare the produced option (and optionally
/// the submitted hashes) to advise on challenging.
pub async fn verify_core(
    config: &RunnerConfig,
    model_config: ModelConfig,
    fetcher: &dyn FactFetcher,
    provider: &dyn AiProvider,
    submitted: &SubmittedClaim,
) -> anyhow::Result<VerifyOutput> {
    let produced = run_core(config, model_config, fetcher, provider).await?;
    let option_matches = produced.option_index == submitted.option;

    let advice = if option_matches {
        "matches (no challenge) — the re-run produced the same categorical option as the submitted claim"
            .to_string()
    } else {
        format!(
            "differs (consider challenging) — re-run produced option {}, submitted claim was option {}",
            produced.option_index, submitted.option
        )
    };

    let check = |submitted: &Option<String>, produced: &str| {
        submitted.as_ref().map(|s| {
            let s_norm = normalize_hex(s);
            HashCheck {
                submitted: s_norm.clone(),
                produced: produced.to_string(),
                matches: s_norm == produced,
            }
        })
    };

    Ok(VerifyOutput {
        submitted_option: submitted.option,
        option_matches,
        advice,
        model_id_check: check(&submitted.model_id_hex, &produced.model_id_hex),
        params_hash_check: check(&submitted.params_hash_hex, &produced.params_hash_hex),
        io_hash_check: check(&submitted.io_hash_hex, &produced.io_hash_hex),
        produced,
    })
}

// --- helpers ----------------------------------------------------------------

/// Normalize a hex string for comparison (strip `0x`, lowercase).
fn normalize_hex(s: &str) -> String {
    s.strip_prefix("0x").unwrap_or(s).to_ascii_lowercase()
}

// --- --submit keeper mode ----------------------------------------------------

/// The validated `--submit` target: the RPC url + keypair path to load + the
/// oracle to submit against. `None` when `--submit` is not set (emit-only, the
/// default).
#[derive(Debug)]
pub(crate) struct SubmitTarget {
    pub(crate) rpc_url: String,
    pub(crate) keypair_path: PathBuf,
    pub(crate) oracle: Pubkey,
}

/// Resolve the oracle pubkey for submission: the explicit `--oracle` (on-chain
/// mode) or the config's `oracle` field (explicit-config mode). Errors clearly
/// if neither is present or the value is not a valid base58 pubkey.
fn resolve_submit_oracle(common: &CommonArgs, config: &RunnerConfig) -> anyhow::Result<Pubkey> {
    let raw = common
        .oracle
        .as_deref()
        .or(config.oracle.as_deref())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "--submit needs an oracle: pass --oracle <pubkey> or set `oracle` in the config"
            )
        })?;
    Pubkey::from_str(raw).map_err(|e| anyhow::anyhow!("invalid oracle pubkey `{raw}`: {e}"))
}

/// Validate the `--submit` args and resolve the submission target.
///
/// `--submit` requires BOTH `--keypair <path>` (the proposer's authority) and
/// `--rpc-url <url>` (the network to submit to — already required in on-chain
/// `--oracle` mode, but ALSO required for submission in explicit-config mode).
/// The two required-arg checks are ordered keypair-then-rpc-url so each surfaces
/// a distinct, clear error. Returns `None` when `--submit` is off.
pub(crate) fn resolve_submit_target(
    common: &CommonArgs,
    submit: bool,
    keypair: Option<&Path>,
    config: &RunnerConfig,
) -> anyhow::Result<Option<SubmitTarget>> {
    if !submit {
        return Ok(None);
    }
    let keypair_path = keypair
        .ok_or_else(|| anyhow::anyhow!("--submit requires --keypair <path>"))?
        .to_path_buf();
    let rpc_url = common.rpc_url.clone().ok_or_else(|| {
        anyhow::anyhow!("--submit requires --rpc-url <url> (the network to submit the claim to)")
    })?;
    let oracle = resolve_submit_oracle(common, config)?;
    Ok(Some(SubmitTarget {
        rpc_url,
        keypair_path,
        oracle,
    }))
}

/// Sign + submit + confirm the run's claim over `rpc` — the testable seam
/// (takes a `&dyn JsonRpc` so the keeper flow runs OFFLINE against
/// [`crate::rpc::MockRpc`], mirroring [`build_config_from_chain`]).
///
/// The submitted transaction carries the RunOutput's OWN 97-byte `payload`
/// verbatim (REUSE — never recomputed), signed by `authority`; the Proposer PDA
/// is DERIVED from `[b"proposer", oracle, authority]`.
pub async fn submit_claim(
    rpc: &dyn crate::rpc::JsonRpc,
    oracle: &Pubkey,
    authority: &Keypair,
    payload: &[u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN],
    opts: ConfirmOptions,
) -> Result<SubmissionOutput, SubmitError> {
    let authority_pubkey = authority.pubkey();
    let proposer = derive_proposer_pda(oracle, &authority_pubkey);
    let confirmation = submit_and_confirm(rpc, oracle, &proposer, authority, payload, opts).await?;
    Ok(SubmissionOutput {
        signature: confirmation.signature,
        confirmation_status: confirmation.confirmation_status,
        oracle: oracle.to_string(),
        proposer: proposer.to_string(),
        authority: authority_pubkey.to_string(),
    })
}
