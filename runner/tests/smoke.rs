//! R0 smoke test: a `CompletionRequest` run through the deterministic
//! `MockProvider` yields the configured option / raw response / model id.

use kassandra_runner::provider::{
    AiProvider, CategoricalOptions, CompletionRequest, MockProvider, ModelConfig,
};

#[tokio::test]
async fn mock_provider_smoke() {
    let req = CompletionRequest {
        system: "Resolve per the oracle interpretation.".to_string(),
        user: "Facts then options 0..3; choose exactly one.".to_string(),
        options: CategoricalOptions {
            count: 3,
            labels: None,
        },
        config: ModelConfig {
            model_id: "claude-opus-4-8".to_string(),
            provider: "anthropic".to_string(),
            max_tokens: 2048,
            thinking: Some("adaptive".to_string()),
        },
    };

    let provider = MockProvider::new(2, r#"{"option_index":2}"#, "mock-claude-opus");
    let resp = provider.complete(&req).await.expect("mock never errors");

    assert_eq!(resp.option_index, 2);
    assert_eq!(resp.raw_response, r#"{"option_index":2}"#);
    assert_eq!(resp.model_id, "mock-claude-opus");
    // The chosen index is within the categorical space.
    assert!(resp.option_index < req.options.count);
}
