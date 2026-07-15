use crate::anthropic::*;
use crate::provider::{AiProvider, CategoricalOptions, CompletionRequest, ModelConfig};

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
    let provider =
        AnthropicProvider::from_env().expect("ANTHROPIC_API_KEY must be set to run the live test");
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
