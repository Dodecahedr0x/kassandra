//! Wire-format helpers: request-body construction and response parsing.
//!
//! These are **pure** (no network) so they are unit-testable with canned JSON.

use serde_json::Value;

use crate::prompt::{output_schema, parse_option_index};
use crate::provider::CompletionRequest;

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
