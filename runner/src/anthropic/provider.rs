//! The [`AnthropicProvider`] itself: pinned constants, URL resolution, the HTTP
//! client wrapper, and the [`AiProvider`] network-call orchestration.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::anthropic::wire::{build_messages_body, parse_messages_response};
use crate::provider::{AiProvider, CompletionRequest, CompletionResponse};

/// The pinned default model string. Centralized here so `params_hash` is stable
/// and the CLI can expose `--model` with this as the default.
pub const DEFAULT_MODEL: &str = "claude-opus-4-8";

/// Default upper bound on generated tokens. Adaptive thinking can add tokens, so
/// this leaves headroom for the (small) JSON answer plus reasoning.
pub const DEFAULT_MAX_TOKENS: u32 = 4096;

/// Thinking mode declared to the API (the only on-mode on Opus 4.8).
pub const THINKING_MODE: &str = "adaptive";

/// Provider identifier folded into `params_hash`.
pub const PROVIDER_ID: &str = "anthropic";

/// The `anthropic-version` header value.
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// The default Messages endpoint (used when no base-URL override is set).
pub const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";

/// Env var that, when set non-empty, overrides the Anthropic API base URL — e.g.
/// to point the REAL provider at a local mock server in tests. Mirrors the
/// official Anthropic SDK's `ANTHROPIC_BASE_URL`: the value is treated as the API
/// **base** and the messages path (`/v1/messages`) is appended. When unset, the
/// pinned [`MESSAGES_URL`] default is used unchanged.
pub const BASE_URL_ENV: &str = "ANTHROPIC_BASE_URL";

/// Resolve the full `/v1/messages` endpoint URL from an optional base-URL
/// override. When `base` is `Some(non-empty)`, the messages path is appended to
/// it (any trailing `/` is trimmed first); otherwise the pinned default
/// [`MESSAGES_URL`] is returned unchanged. Pure so it is unit-testable.
pub fn resolve_messages_url(base: Option<&str>) -> String {
    match base {
        Some(b) if !b.trim().is_empty() => {
            format!("{}/v1/messages", b.trim().trim_end_matches('/'))
        }
        _ => MESSAGES_URL.to_string(),
    }
}

/// The base-URL override from [`BASE_URL_ENV`], if set to a non-empty value.
fn base_url_from_env() -> Option<String> {
    std::env::var(BASE_URL_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// Per-request timeout. Adaptive thinking on hard prompts can take a while, so
/// this is generous.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// The real Anthropic/Claude provider over raw HTTP.
#[derive(Clone)]
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    /// The resolved `/v1/messages` endpoint (the default unless overridden).
    messages_url: String,
}

impl std::fmt::Debug for AnthropicProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print the API key.
        f.debug_struct("AnthropicProvider")
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl AnthropicProvider {
    /// Build a provider from an explicit API key (with the default timeout). The
    /// key is never logged. The endpoint is the default [`MESSAGES_URL`] unless
    /// [`BASE_URL_ENV`] is set, in which case that base is used (see
    /// [`resolve_messages_url`]).
    pub fn new(api_key: impl Into<String>) -> anyhow::Result<Self> {
        Self::build(
            api_key,
            resolve_messages_url(base_url_from_env().as_deref()),
        )
    }

    /// Build a provider with an explicit base-URL override (the messages path is
    /// appended via [`resolve_messages_url`]). Additive helper for tests that
    /// point the REAL provider at a local mock server without touching env.
    pub fn with_base_url(
        api_key: impl Into<String>,
        base_url: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        Self::build(api_key, resolve_messages_url(Some(base_url.as_ref())))
    }

    /// Shared constructor: validates the key and builds the HTTP client.
    fn build(api_key: impl Into<String>, messages_url: String) -> anyhow::Result<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            anyhow::bail!("ANTHROPIC_API_KEY is empty");
        }
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build HTTP client: {e}"))?;
        Ok(Self {
            client,
            api_key,
            messages_url,
        })
    }

    /// The resolved `/v1/messages` endpoint this provider posts to.
    pub fn messages_url(&self) -> &str {
        &self.messages_url
    }

    /// Build a provider reading the API key from `ANTHROPIC_API_KEY`. Errors
    /// clearly (with a `--mock` hint) if the variable is unset or empty. The key
    /// is NEVER hardcoded.
    pub fn from_env() -> anyhow::Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            anyhow::anyhow!(
                "ANTHROPIC_API_KEY is not set. Export it, or run with --mock \
                 (or KASSANDRA_RUNNER_MOCK=1) for offline use."
            )
        })?;
        Self::new(api_key)
    }
}

#[async_trait]
impl AiProvider for AnthropicProvider {
    async fn complete(&self, req: &CompletionRequest) -> anyhow::Result<CompletionResponse> {
        let body = build_messages_body(req);

        let resp = self
            .client
            .post(&self.messages_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Anthropic request transport error: {e}"))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| anyhow::anyhow!("failed to read Anthropic response body: {e}"))?;

        if !status.is_success() {
            anyhow::bail!("Anthropic API returned HTTP {}: {text}", status.as_u16());
        }

        let json: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Anthropic response was not valid JSON: {e}"))?;

        let (raw_response, option_index, model_id) =
            parse_messages_response(&json, &req.config.model_id, req.options.count)?;

        // Keep model_id and params_hash consistent: the resolved model string
        // flows into both.
        let mut params = req.config.clone();
        params.model_id = model_id.clone();

        Ok(CompletionResponse {
            option_index,
            raw_response,
            model_id,
            params,
        })
    }
}
