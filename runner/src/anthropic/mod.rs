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

mod provider;
mod wire;

pub use provider::{
    resolve_messages_url, AnthropicProvider, ANTHROPIC_VERSION, BASE_URL_ENV, DEFAULT_MAX_TOKENS,
    DEFAULT_MODEL, DEFAULT_REQUEST_TIMEOUT, MESSAGES_URL, PROVIDER_ID, THINKING_MODE,
};
pub use wire::{build_messages_body, parse_messages_response};

#[cfg(test)]
mod tests;
