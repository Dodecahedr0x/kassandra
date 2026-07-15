//! Input config types + config resolution (explicit JSON or on-chain build).

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::cli::CommonArgs;
use crate::fetch::{FactRef, HttpFactFetcher};
use crate::provider::{CategoricalOption, CategoricalOptions};

/// Env var that, when set non-empty, forces the MockProvider (offline).
pub const MOCK_ENV: &str = "KASSANDRA_RUNNER_MOCK";

// --- input config -----------------------------------------------------------

/// An agreed fact reference in the input config: `content_hash` (hex) + `uri`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FactInput {
    /// `sha256(content)` as 64 lowercase hex chars (a leading `0x` is allowed).
    pub content_hash: String,
    /// The http/https location the content is served from.
    pub uri: String,
}

/// An optional human-readable label for a categorical option.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OptionLabelInput {
    /// The on-chain option index.
    pub index: u8,
    /// The label text.
    pub label: String,
}

/// The oracle config the CLI consumes (JSON from `--config` or stdin).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RunnerConfig {
    /// The oracle's interpretation / resolution-rule text (its on-chain
    /// `prompt_hash` commitment).
    pub interpretation: String,
    /// The number of categorical options (mirrors `Oracle.options_count`).
    pub options_count: u8,
    /// Optional per-option labels.
    #[serde(default)]
    pub option_labels: Option<Vec<OptionLabelInput>>,
    /// The agreed facts (each `content_hash` hex + `uri`).
    #[serde(default)]
    pub facts: Vec<FactInput>,
    /// Optional oracle pubkey — only echoed to describe the AiClaim PDA seeds.
    #[serde(default)]
    pub oracle: Option<String>,
    /// Optional proposer pubkey — only echoed to describe the AiClaim PDA seeds.
    #[serde(default)]
    pub proposer: Option<String>,
}

impl RunnerConfig {
    /// Parse `facts` into verifiable [`FactRef`]s.
    pub(crate) fn fact_refs(&self) -> anyhow::Result<Vec<FactRef>> {
        self.facts
            .iter()
            .map(|f| Ok(FactRef::new(parse_hex32(&f.content_hash)?, f.uri.clone())))
            .collect()
    }

    /// Build the categorical answer space from `options_count` + `option_labels`.
    pub(crate) fn categorical_options(&self) -> CategoricalOptions {
        let labels = self.option_labels.as_ref().map(|ls| {
            ls.iter()
                .map(|l| CategoricalOption {
                    index: l.index,
                    label: Some(l.label.clone()),
                })
                .collect()
        });
        CategoricalOptions {
            count: self.options_count,
            labels,
        }
    }
}

// --- helpers ----------------------------------------------------------------

/// Parse exactly 32 bytes from a 64-char hex string (optional `0x` prefix).
pub(crate) fn parse_hex32(s: &str) -> anyhow::Result<[u8; 32]> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes =
        hex::decode(s).map_err(|e| anyhow::anyhow!("invalid hex in content_hash: {e}"))?;
    bytes.as_slice().try_into().map_err(|_| {
        anyhow::anyhow!(
            "content_hash must be 32 bytes (64 hex chars), got {} bytes",
            bytes.len()
        )
    })
}

/// Load the config from `--config <path>` or, when `None`, stdin.
fn load_config(path: Option<&Path>) -> anyhow::Result<RunnerConfig> {
    let text = match path {
        Some(p) => std::fs::read_to_string(p)
            .map_err(|e| anyhow::anyhow!("failed to read config `{}`: {e}", p.display()))?,
        None => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| anyhow::anyhow!("failed to read config from stdin: {e}"))?;
            buf
        }
    };
    serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("invalid config JSON: {e}"))
}

/// Build a [`RunnerConfig`] by reading the oracle + its agreed facts over RPC
/// and pairing them with a verified off-chain interpretation.
///
/// Generic over [`crate::rpc::JsonRpc`] so it is testable offline with
/// [`crate::rpc::MockRpc`]. Fetches the `Oracle` (owner + `AccountType` tag
/// verified, then Pod-decoded via the shared struct), asserts
/// `sha256(prompt_text) == oracle.prompt_hash` (REJECTS a mismatch), enumerates
/// the AGREED facts, and assembles the config: `options_count`/`deadline`-backed
/// facts from chain, the interpretation from the prompt text.
/// The subset of the oracle-metadata JSON the runner consumes. Fetched from the
/// on-chain `oracle_meta.uri` and verified against `uri_hash`. `promptTemplate`
/// is the AI-runner interpretation (defaulted at creation); `interpretation` is
/// the optional human rules — the runner prefers the former.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OracleMetaJson {
    #[serde(default)]
    prompt_template: Option<String>,
    #[serde(default)]
    interpretation: Option<String>,
}

pub async fn build_config_from_chain(
    rpc: &dyn crate::rpc::JsonRpc,
    fetcher: &dyn crate::fetch::FactFetcher,
    oracle_pubkey: &str,
) -> anyhow::Result<RunnerConfig> {
    // 1. Read the on-chain metadata (subject/options/uri/uri_hash).
    let meta = crate::rpc::fetch_oracle_meta(rpc, oracle_pubkey).await?;
    if meta.uri.is_empty() {
        anyhow::bail!(
            "oracle `{oracle_pubkey}` has no metadata uri on chain — cannot read the interpretation"
        );
    }

    // 2. Fetch the metadata JSON and VERIFY it against the on-chain uri_hash
    //    (exactly the fact content-hash contract: fetch by uri, check the hash).
    let json_bytes = fetcher.fetch(&meta.uri).await.map_err(|e| {
        anyhow::anyhow!(
            "failed to fetch oracle metadata JSON at `{}`: {e}",
            meta.uri
        )
    })?;
    let actual: [u8; 32] = Sha256::digest(&json_bytes).into();
    if actual != meta.uri_hash {
        anyhow::bail!(
            "oracle metadata JSON at `{}` hashes to sha256 {} but the on-chain uri_hash is {} \
             (the hosted JSON does not match what this oracle committed to)",
            meta.uri,
            hex::encode(&actual),
            hex::encode(&meta.uri_hash),
        );
    }

    // 3. Parse + take the interpretation (promptTemplate preferred).
    let meta_json: OracleMetaJson = serde_json::from_slice(&json_bytes)
        .map_err(|e| anyhow::anyhow!("oracle metadata JSON is malformed: {e}"))?;
    let interpretation = meta_json
        .prompt_template
        .filter(|s| !s.trim().is_empty())
        .or_else(|| meta_json.interpretation.filter(|s| !s.trim().is_empty()))
        .ok_or_else(|| {
            anyhow::anyhow!("oracle metadata JSON has no promptTemplate/interpretation text")
        })?;

    // Option labels come straight from the on-chain (program-readable) labels.
    let option_labels = (!meta.options.is_empty()).then(|| {
        meta.options
            .iter()
            .enumerate()
            .map(|(i, label)| OptionLabelInput {
                index: i as u8,
                label: label.clone(),
            })
            .collect()
    });

    // 4. Agreed facts (unchanged).
    let facts = crate::rpc::fetch_agreed_facts(rpc, oracle_pubkey).await?;
    let facts = facts
        .into_iter()
        .map(|f| FactInput {
            content_hash: hex::encode(&f.content_hash),
            uri: f.uri,
        })
        .collect();

    Ok(RunnerConfig {
        interpretation,
        options_count: meta.options.len() as u8,
        option_labels,
        facts,
        oracle: Some(oracle_pubkey.to_string()),
        proposer: None,
    })
}

/// Resolve the [`RunnerConfig`] for a command from its [`CommonArgs`]: either
/// the explicit JSON config (`--config`/stdin) or the on-chain fetch
/// (`--oracle` + `--rpc-url` + `--prompt-file`). The two modes are mutually
/// exclusive.
pub(crate) async fn resolve_config(common: &CommonArgs) -> anyhow::Result<RunnerConfig> {
    match &common.oracle {
        Some(oracle) => {
            if common.config.is_some() {
                anyhow::bail!("--oracle and --config are mutually exclusive");
            }
            let rpc_url = common
                .rpc_url
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--oracle requires --rpc-url <url>"))?;
            // The interpretation is read from chain (oracle_meta.uri → JSON,
            // verified against uri_hash) — no --prompt-file needed.
            let rpc = crate::rpc::HttpJsonRpc::new(rpc_url.clone())?;
            let fetcher = HttpFactFetcher::new()?;
            build_config_from_chain(&rpc, &fetcher, oracle).await
        }
        None => load_config(common.config.as_deref()),
    }
}

/// Whether to use the mock provider (the `--mock` flag or `KASSANDRA_RUNNER_MOCK`
/// set non-empty).
pub(crate) fn use_mock(flag: bool) -> bool {
    flag || std::env::var(MOCK_ENV)
        .map(|v| !v.is_empty())
        .unwrap_or(false)
}
