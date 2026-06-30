//! The default Anthropic (Claude) provider (Task R4).
//!
//! Rust has no official Anthropic SDK, so this is a thin `reqwest`-based client
//! over `POST https://api.anthropic.com/v1/messages`. It implements the generic
//! [`AiProvider`] trait so the rest of the runner (and the CLI) stays
//! model-agnostic and can swap in the [`crate::provider::MockProvider`] offline.
//!
//! # Request body (the pinned contract)
//!
//! [`build_messages_body`] constructs exactly:
//!
//! ```json
//! {
//!   "model": "claude-opus-4-8",
//!   "max_tokens": <config.max_tokens>,
//!   "thinking": { "type": "adaptive" },
//!   "system": "<assembled system>",
//!   "messages": [{ "role": "user", "content": "<assembled user>" }],
//!   "output_config": { "format": { "type": "json_schema", "schema": <output_schema(count)> } }
//! }
//! ```
//!
//! It deliberately does **not** send `temperature` / `top_p` / `top_k` /
//! `budget_tokens` — Opus 4.8 rejects all of those with a 400. Adaptive thinking
//! is the only on-mode on 4.8; the categorical answer is forced via structured
//! output ([`crate::prompt::output_schema`]) rather than free-text scraping.
//!
//! # Capturing `raw_response` VERBATIM
//!
//! `io_hash` commits to the model's raw structured-output text byte-for-byte, so
//! [`parse_messages_response`] concatenates the `.text` of the response's `text`
//! content block(s) **without re-serializing** and stores that exact string as
//! [`CompletionResponse::raw_response`]. (With structured output there is one
//! text block whose text is the answer JSON; thinking blocks are skipped.) Only
//! after capturing the verbatim text do we parse it via
//! [`crate::prompt::parse_option_index`].
//!
//! # Resolved `model_id` (proposer/challenger must agree)
//!
//! Both the proposer and a challenger pin the request to the same model string
//! (`claude-opus-4-8` by default). For fidelity we set
//! [`CompletionResponse::model_id`] to the response's `model` field when present,
//! falling back to the requested string otherwise — and copy that resolved value
//! into [`CompletionResponse::params`] so `model_id` and `params_hash` are
//! computed from the **same** string. Because both parties request the same
//! pinned model and the API echoes it back verbatim, both derive the same
//! `model_id`. (Frontier APIs are not bit-reproducible; this is best-effort
//! determinism per the design — `io_hash` is a commitment to what the submitter
//! actually saw, not a reproducibility oracle.)

use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::prompt::{output_schema, parse_option_index};
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

/// Build the exact `/v1/messages` request body for `req`.
///
/// See the module docs for the pinned shape. Notably it sends `thinking` only
/// when `req.config.thinking` is `Some` and never sends sampling params or
/// `budget_tokens`.
pub fn build_messages_body(req: &CompletionRequest) -> Value {
    let mut body = serde_json::json!({
        "model": req.config.model_id,
        "max_tokens": req.config.max_tokens,
        "system": req.system,
        "messages": [{ "role": "user", "content": req.user }],
        "output_config": {
            "format": {
                "type": "json_schema",
                "schema": output_schema(req.options.count),
            }
        }
    });
    if let Some(mode) = req.config.thinking.as_deref() {
        body["thinking"] = serde_json::json!({ "type": mode });
    }
    body
}

/// Resolve the model id to record: the response's `model` field if present, else
/// the requested string. Documented choice — see the module docs.
fn resolve_model_id(body: &Value, requested_model: &str) -> String {
    body.get("model")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or(requested_model)
        .to_string()
}

/// Parse a `/v1/messages` response body into `(raw_response, option_index,
/// model_id)`.
///
/// This is a **pure** function (no network) so the response handling is unit
/// testable with canned JSON. It:
/// 1. errors clearly on `stop_reason == "refusal"` (without trying to parse
///    content), surfacing the `stop_details` category/explanation;
/// 2. captures the verbatim concatenation of all `text` content blocks as
///    `raw_response` (no re-serialization);
/// 3. parses that text via [`parse_option_index`] against `options_count`;
/// 4. resolves the model id via [`resolve_model_id`].
pub fn parse_messages_response(
    body: &Value,
    requested_model: &str,
    options_count: u8,
) -> anyhow::Result<(String, u8, String)> {
    if body.get("stop_reason").and_then(Value::as_str) == Some("refusal") {
        let details = body.get("stop_details");
        let category = details
            .and_then(|d| d.get("category"))
            .and_then(Value::as_str)
            .unwrap_or("unspecified");
        let explanation = details
            .and_then(|d| d.get("explanation"))
            .and_then(Value::as_str)
            .unwrap_or("");
        anyhow::bail!(
            "Anthropic declined the request (stop_reason=refusal, category={category}): {explanation}"
        );
    }

    let content = body
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("Anthropic response is missing the `content` array"))?;

    // VERBATIM capture: concatenate the text block(s) exactly as returned.
    let mut raw_response = String::new();
    for block in content {
        if block.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(text) = block.get("text").and_then(Value::as_str) {
                raw_response.push_str(text);
            }
        }
    }
    if raw_response.is_empty() {
        anyhow::bail!(
            "Anthropic response contained no text/structured-output block (stop_reason={:?})",
            body.get("stop_reason").and_then(Value::as_str)
        );
    }

    let option_index = parse_option_index(&raw_response, options_count)
        .map_err(|e| anyhow::anyhow!("failed to parse structured output `{raw_response}`: {e}"))?;

    let model_id = resolve_model_id(body, requested_model);
    Ok((raw_response, option_index, model_id))
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{CategoricalOptions, ModelConfig};

    fn sample_request() -> CompletionRequest {
        CompletionRequest {
            system: "Decide per the rules.".to_string(),
            user: "Facts...\n[0] yes\n[1] no".to_string(),
            options: CategoricalOptions {
                count: 2,
                labels: None,
            },
            config: ModelConfig {
                model_id: DEFAULT_MODEL.to_string(),
                provider: PROVIDER_ID.to_string(),
                max_tokens: DEFAULT_MAX_TOKENS,
                thinking: Some(THINKING_MODE.to_string()),
            },
        }
    }

    // --- request body construction -----------------------------------------

    #[test]
    fn body_has_pinned_shape() {
        let body = build_messages_body(&sample_request());
        assert_eq!(body["model"], DEFAULT_MODEL);
        assert_eq!(body["max_tokens"], DEFAULT_MAX_TOKENS);
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert_eq!(body["system"], "Decide per the rules.");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "Facts...\n[0] yes\n[1] no");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        // The schema's maximum is options_count - 1.
        assert_eq!(
            body["output_config"]["format"]["schema"]["properties"]["option_index"]["maximum"],
            1
        );
    }

    #[test]
    fn body_omits_sampling_and_budget_params() {
        let body = build_messages_body(&sample_request());
        // Opus 4.8 rejects these with a 400 — they must never be sent.
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
        assert!(body.get("top_k").is_none());
        assert!(body.get("budget_tokens").is_none());
        // thinking is adaptive only — no nested budget_tokens.
        assert!(body["thinking"].get("budget_tokens").is_none());
    }

    #[test]
    fn body_omits_thinking_when_none() {
        let mut req = sample_request();
        req.config.thinking = None;
        let body = build_messages_body(&req);
        assert!(body.get("thinking").is_none());
    }

    // --- response parsing (offline, canned JSON) ---------------------------

    #[test]
    fn parse_extracts_verbatim_text_and_index() {
        // A thinking block precedes the structured-output text block.
        let body = serde_json::json!({
            "model": "claude-opus-4-8",
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "reasoning..." },
                { "type": "text", "text": "{\"option_index\": 1}" }
            ]
        });
        let (raw, idx, model) = parse_messages_response(&body, DEFAULT_MODEL, 2).unwrap();
        // VERBATIM: exactly the text block's bytes, no re-serialization.
        assert_eq!(raw, "{\"option_index\": 1}");
        assert_eq!(idx, 1);
        assert_eq!(model, "claude-opus-4-8");
    }

    #[test]
    fn parse_uses_response_model_when_present_else_requested() {
        let with_model = serde_json::json!({
            "model": "claude-opus-4-8",
            "content": [{ "type": "text", "text": "{\"option_index\":0}" }]
        });
        let (_, _, m) = parse_messages_response(&with_model, "requested-fallback", 2).unwrap();
        assert_eq!(m, "claude-opus-4-8");

        let without_model = serde_json::json!({
            "content": [{ "type": "text", "text": "{\"option_index\":0}" }]
        });
        let (_, _, m) = parse_messages_response(&without_model, "requested-fallback", 2).unwrap();
        assert_eq!(m, "requested-fallback");
    }

    #[test]
    fn parse_rejects_refusal() {
        let body = serde_json::json!({
            "stop_reason": "refusal",
            "stop_details": { "category": "cyber", "explanation": "no" },
            "content": []
        });
        let err = parse_messages_response(&body, DEFAULT_MODEL, 2).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("refusal"), "{msg}");
        assert!(msg.contains("cyber"), "{msg}");
    }

    #[test]
    fn parse_rejects_missing_content() {
        let body = serde_json::json!({ "stop_reason": "end_turn" });
        assert!(parse_messages_response(&body, DEFAULT_MODEL, 2).is_err());
    }

    #[test]
    fn parse_rejects_no_text_block() {
        let body = serde_json::json!({
            "stop_reason": "end_turn",
            "content": [{ "type": "thinking", "thinking": "..." }]
        });
        assert!(parse_messages_response(&body, DEFAULT_MODEL, 2).is_err());
    }

    #[test]
    fn parse_rejects_out_of_range_index() {
        let body = serde_json::json!({
            "model": "claude-opus-4-8",
            "content": [{ "type": "text", "text": "{\"option_index\": 5}" }]
        });
        assert!(parse_messages_response(&body, DEFAULT_MODEL, 2).is_err());
    }

    #[test]
    fn new_rejects_empty_key() {
        assert!(AnthropicProvider::new("   ").is_err());
        assert!(AnthropicProvider::new("sk-test-key").is_ok());
    }

    // --- base-URL override (additive; default unchanged) -------------------

    #[test]
    fn resolve_messages_url_default_when_unset_or_empty() {
        // Unset → the pinned default, unchanged.
        assert_eq!(resolve_messages_url(None), MESSAGES_URL);
        // Empty / whitespace-only → still the default (treated as not set).
        assert_eq!(resolve_messages_url(Some("")), MESSAGES_URL);
        assert_eq!(resolve_messages_url(Some("   ")), MESSAGES_URL);
    }

    #[test]
    fn resolve_messages_url_appends_path_to_override() {
        // The override is the API base; the messages path is appended.
        assert_eq!(
            resolve_messages_url(Some("http://127.0.0.1:8989")),
            "http://127.0.0.1:8989/v1/messages"
        );
        // A trailing slash on the base is trimmed (no doubled `//`).
        assert_eq!(
            resolve_messages_url(Some("http://127.0.0.1:8989/")),
            "http://127.0.0.1:8989/v1/messages"
        );
        // Surrounding whitespace is ignored.
        assert_eq!(
            resolve_messages_url(Some("  http://localhost:1234  ")),
            "http://localhost:1234/v1/messages"
        );
    }

    #[test]
    fn provider_honors_base_url_override_and_keeps_default() {
        // Default constructor (no env in this test) → the pinned default URL.
        let default = AnthropicProvider::new("sk-test-key").unwrap();
        assert_eq!(default.messages_url(), MESSAGES_URL);

        // Explicit override → the mock endpoint, leaving the default untouched.
        let overridden =
            AnthropicProvider::with_base_url("sk-test-key", "http://127.0.0.1:8989").unwrap();
        assert_eq!(
            overridden.messages_url(),
            "http://127.0.0.1:8989/v1/messages"
        );
    }

    // --- live integration test: env-gated + #[ignore] ----------------------
    // Never runs in the normal suite (no key required). Run manually with:
    //   ANTHROPIC_API_KEY=sk-... cargo test -p kassandra-runner --lib \
    //     -- --ignored live_anthropic_completion --nocapture
    #[tokio::test]
    #[ignore = "requires ANTHROPIC_API_KEY + network; run manually with --ignored"]
    async fn live_anthropic_completion() {
        let provider = AnthropicProvider::from_env()
            .expect("ANTHROPIC_API_KEY must be set to run the live test");
        let req = sample_request();
        let resp = provider
            .complete(&req)
            .await
            .expect("live completion failed");
        assert!(
            resp.option_index < req.options.count,
            "option {} out of range",
            resp.option_index
        );
        assert!(!resp.raw_response.is_empty());
        assert_eq!(resp.model_id, resp.params.model_id);
    }
}
