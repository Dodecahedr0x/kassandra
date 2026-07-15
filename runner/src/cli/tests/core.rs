use super::{parse_payload, sample_config};
use crate::cli::{build_model_config, parse_hex32, run_core, verify_core, SubmittedClaim};
use crate::fetch::MockFactFetcher;
use crate::provider::MockProvider;

#[test]
fn parse_hex32_roundtrips() {
    let bytes = [0xabu8; 32];
    let encoded = hex::encode(bytes);
    assert_eq!(parse_hex32(&encoded).unwrap(), bytes);
    assert_eq!(parse_hex32(&format!("0x{encoded}")).unwrap(), bytes);
    assert!(parse_hex32("abcd").is_err());
    assert!(parse_hex32(&"zz".repeat(32)).is_err());
}

#[tokio::test]
async fn run_core_with_mocks_emits_97_byte_payload() {
    let content = b"BTC closed at $98,000.";
    let uri = "https://facts.example/btc";
    let config = sample_config(uri, content);
    let fetcher = MockFactFetcher::new().with(uri, content.to_vec());
    let provider = MockProvider::new(1, r#"{"option_index":1}"#, "mock-claude");
    let model_config = build_model_config(None, None);

    let out = run_core(&config, model_config, &fetcher, &provider)
        .await
        .unwrap();

    // Option matches the mock.
    assert_eq!(out.option_index, 1);

    // Payload is exactly 97 bytes: model_id[32] ++ params_hash[32] ++
    // io_hash[32] ++ option[1].
    let payload = parse_payload(&out.submit_ai_claim_payload_hex);
    assert_eq!(payload.len(), 97);
    assert_eq!(hex::encode(&payload[0..32]), out.model_id_hex);
    assert_eq!(hex::encode(&payload[32..64]), out.params_hash_hex);
    assert_eq!(hex::encode(&payload[64..96]), out.io_hash_hex);
    assert_eq!(payload[96], 1);

    // The resolved model id is the mock's.
    assert_eq!(out.resolved_model_id, "mock-claude");
}

#[tokio::test]
async fn run_core_rejects_tampered_fact() {
    let committed = b"the agreed fact";
    let tampered = b"a tampered fact";
    let uri = "https://facts.example/x";
    let config = sample_config(uri, committed);
    // Serve tampered bytes that don't match the committed content_hash.
    let fetcher = MockFactFetcher::new().with(uri, tampered.to_vec());
    let provider = MockProvider::default();

    let err = run_core(&config, build_model_config(None, None), &fetcher, &provider)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("content_hash mismatch"), "{err}");
}

#[tokio::test]
async fn verify_core_reports_match_and_mismatch() {
    let content = b"some fact";
    let uri = "https://facts.example/y";
    let config = sample_config(uri, content);
    let fetcher = MockFactFetcher::new().with(uri, content.to_vec());
    let provider = MockProvider::new(0, r#"{"option_index":0}"#, "mock-claude");

    // Matching submitted option.
    let matching = SubmittedClaim {
        option: 0,
        ..Default::default()
    };
    let out = verify_core(
        &config,
        build_model_config(None, None),
        &fetcher,
        &provider,
        &matching,
    )
    .await
    .unwrap();
    assert!(out.option_matches);
    assert!(out.advice.contains("matches"));

    // Differing submitted option.
    let differing = SubmittedClaim {
        option: 1,
        ..Default::default()
    };
    let out = verify_core(
        &config,
        build_model_config(None, None),
        &fetcher,
        &provider,
        &differing,
    )
    .await
    .unwrap();
    assert!(!out.option_matches);
    assert!(out.advice.contains("differs"));
}

#[tokio::test]
async fn verify_core_compares_submitted_hashes() {
    let content = b"a fact";
    let uri = "https://facts.example/z";
    let config = sample_config(uri, content);
    let fetcher = MockFactFetcher::new().with(uri, content.to_vec());
    let provider = MockProvider::new(0, r#"{"option_index":0}"#, "mock-claude");

    // First produce the real hashes via run_core.
    let produced = run_core(&config, build_model_config(None, None), &fetcher, &provider)
        .await
        .unwrap();

    let submitted = SubmittedClaim {
        option: 0,
        model_id_hex: Some(produced.model_id_hex.clone()),
        params_hash_hex: Some(produced.params_hash_hex.clone()),
        io_hash_hex: Some("deadbeef".to_string()), // intentionally wrong
    };
    let out = verify_core(
        &config,
        build_model_config(None, None),
        &fetcher,
        &provider,
        &submitted,
    )
    .await
    .unwrap();

    assert!(out.model_id_check.unwrap().matches);
    assert!(out.params_hash_check.unwrap().matches);
    assert!(!out.io_hash_check.unwrap().matches);
}
