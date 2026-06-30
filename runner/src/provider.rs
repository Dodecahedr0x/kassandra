//! The generic AI provider abstraction.
//!
//! [`AiProvider`] hides the concrete model behind a single `complete` call so
//! the rest of the runner is model-agnostic. The real Anthropic/Claude provider
//! (Task R4) and the deterministic [`MockProvider`] (the test default) both
//! implement it.
//!
//! Types here are intentionally provider-agnostic: a [`CompletionRequest`]
//! carries the already-assembled model input + the categorical options + a
//! [`ModelConfig`]; a [`CompletionResponse`] carries the chosen option index,
//! the verbatim raw response, and enough of the resolved config for Task R1 to
//! compute the canonical `params_hash` / `io_hash`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// The model + request configuration that affects the answer. The fields that
/// matter for reproducibility feed `params_hash` in Task R1, so keep this a
/// stable, declared description of how the model was called.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelConfig {
    /// The pinned model identifier string (e.g. `"claude-opus-4-8"`). Hashed
    /// into the on-chain `model_id` in Task R1.
    pub model_id: String,
    /// Provider identifier (e.g. `"anthropic"` / `"mock"`), part of the
    /// declared params so a challenger reproduces the same provider path.
    pub provider: String,
    /// Upper bound on generated tokens.
    pub max_tokens: u32,
    /// Thinking mode declared to the provider (e.g. `"adaptive"`), if any.
    pub thinking: Option<String>,
}

/// One categorical option the model must choose between.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoricalOption {
    /// The on-chain option index (`0..options_count`).
    pub index: u8,
    /// Optional human-readable label for the option (may be `None` if the
    /// oracle defines options only positionally).
    pub label: Option<String>,
}

/// The categorical answer space: the number of options plus optional labels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoricalOptions {
    /// Total number of options (mirrors `Oracle.options_count`). The chosen
    /// index must satisfy `0 <= index < count`.
    pub count: u8,
    /// Optional per-option labels. When present, length must equal `count`.
    pub labels: Option<Vec<CategoricalOption>>,
}

/// A fully-assembled completion request: the model input (system + user text),
/// the categorical answer space, and the model config. Provider-agnostic — the
/// prompt assembly that builds `system`/`user` lives in Task R2.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// System text: the oracle's fixed interpretation/prompt.
    pub system: String,
    /// User text: the agreed facts (canonical order) + enumerated options +
    /// the choose-one instruction.
    pub user: String,
    /// The categorical options the model must pick from.
    pub options: CategoricalOptions,
    /// How to call the model.
    pub config: ModelConfig,
}

/// The provider's answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// The chosen categorical option index (`0..options_count`). Validation
    /// against `options.count` is the caller's responsibility (Task R2).
    pub option_index: u8,
    /// The model's raw response text, VERBATIM (the structured-output JSON
    /// string for the real provider). Hashed into `io_hash` in Task R1, so it
    /// must be the exact bytes the submitter saw.
    pub raw_response: String,
    /// The resolved model identifier string, as actually used.
    pub model_id: String,
    /// The model config actually used (enough for R1 to hash `params_hash`).
    pub params: ModelConfig,
}

/// The generic AI provider. One async call from an assembled request to a
/// categorical answer + the metadata Task R1 hashes.
#[async_trait]
pub trait AiProvider {
    /// Run the request through the model and return the chosen option + the
    /// verbatim raw response + the resolved config.
    async fn complete(&self, req: &CompletionRequest) -> anyhow::Result<CompletionResponse>;
}

/// A deterministic, no-network provider: returns a fixed option index, a canned
/// raw response, and a fixed model id regardless of the request. The default
/// for tests and offline runs (the CLI's `--mock`).
#[derive(Clone, Debug)]
pub struct MockProvider {
    /// The option index every `complete` returns.
    pub option_index: u8,
    /// The verbatim raw response every `complete` returns.
    pub raw_response: String,
    /// The model id reported in the response.
    pub model_id: String,
}

impl MockProvider {
    /// Construct a mock with the given fixed outputs.
    pub fn new(
        option_index: u8,
        raw_response: impl Into<String>,
        model_id: impl Into<String>,
    ) -> Self {
        Self {
            option_index,
            raw_response: raw_response.into(),
            model_id: model_id.into(),
        }
    }
}

impl Default for MockProvider {
    /// A canonical deterministic default: option 0 and a minimal JSON answer.
    fn default() -> Self {
        Self::new(0, r#"{"option_index":0}"#, "mock-model")
    }
}

#[async_trait]
impl AiProvider for MockProvider {
    async fn complete(&self, req: &CompletionRequest) -> anyhow::Result<CompletionResponse> {
        // Deterministic: ignore the request content, echo the configured
        // params back so downstream hashing sees a stable picture.
        let mut params = req.config.clone();
        params.model_id = self.model_id.clone();
        Ok(CompletionResponse {
            option_index: self.option_index,
            raw_response: self.raw_response.clone(),
            model_id: self.model_id.clone(),
            params,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> CompletionRequest {
        CompletionRequest {
            system: "Decide the outcome per the interpretation.".to_string(),
            user: "Facts: ...\nOptions:\n0) yes\n1) no\nChoose exactly one.".to_string(),
            options: CategoricalOptions {
                count: 2,
                labels: Some(vec![
                    CategoricalOption {
                        index: 0,
                        label: Some("yes".to_string()),
                    },
                    CategoricalOption {
                        index: 1,
                        label: Some("no".to_string()),
                    },
                ]),
            },
            config: ModelConfig {
                model_id: "claude-opus-4-8".to_string(),
                provider: "anthropic".to_string(),
                max_tokens: 1024,
                thinking: Some("adaptive".to_string()),
            },
        }
    }

    #[tokio::test]
    async fn mock_provider_is_deterministic() {
        let provider = MockProvider::new(1, r#"{"option_index":1}"#, "mock-claude");
        let req = sample_request();

        let r1 = provider.complete(&req).await.unwrap();
        let r2 = provider.complete(&req).await.unwrap();

        assert_eq!(r1.option_index, 1);
        assert_eq!(r1.raw_response, r#"{"option_index":1}"#);
        assert_eq!(r1.model_id, "mock-claude");
        // The resolved params carry the mock's model id, not the request's.
        assert_eq!(r1.params.model_id, "mock-claude");
        // Determinism: identical request -> identical response.
        assert_eq!(r1, r2);
    }

    #[tokio::test]
    async fn mock_default_returns_option_zero() {
        let provider = MockProvider::default();
        let resp = provider.complete(&sample_request()).await.unwrap();
        assert_eq!(resp.option_index, 0);
        assert_eq!(resp.raw_response, r#"{"option_index":0}"#);
    }
}
