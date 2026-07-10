use crate::constants::{IO_HASH_LEN, MODEL_ID_LEN, PARAMS_HASH_LEN, SUBMIT_AI_CLAIM_PAYLOAD_LEN};
use crate::hashing::*;
use crate::provider::{CategoricalOptions, CompletionRequest, CompletionResponse, ModelConfig};

fn sample_config() -> ModelConfig {
    ModelConfig {
        model_id: "claude-opus-4-8".to_string(),
        provider: "anthropic".to_string(),
        max_tokens: 1024,
        thinking: Some("adaptive".to_string()),
    }
}

fn sample_request() -> CompletionRequest {
    CompletionRequest {
        system: "Decide the outcome per the interpretation.".to_string(),
        user: "Facts: ...\nOptions:\n0) yes\n1) no\nChoose exactly one.".to_string(),
        options: CategoricalOptions {
            count: 2,
            labels: None,
        },
        config: sample_config(),
    }
}

fn sample_response() -> CompletionResponse {
    CompletionResponse {
        option_index: 1,
        raw_response: r#"{"option_index":1}"#.to_string(),
        model_id: "claude-opus-4-8".to_string(),
        params: sample_config(),
    }
}

// --- known-answer / regression anchors ---------------------------------
// Computed once from the fixed inputs above and pinned. A change to ANY
// input (or to the encoding) flips the corresponding anchor, which is the
// signal a challenger would use to detect a divergent runner.

#[test]
fn model_id_known_answer() {
    // sha256("claude-opus-4-8"), cross-checked with `shasum -a 256`.
    assert_eq!(
        hex::encode(hash_model_id("claude-opus-4-8")),
        "47a46a22f0c9fb105db3f0d8bda83ad51bd59369ab8c8c30cc32ba6356ac5a4a"
    );
}

#[test]
fn params_hash_known_answer() {
    let config = sample_config();
    let params = CanonicalParams::from_config(&config);
    assert_eq!(
        hex::encode(hash_params(&params)),
        "a08e048d8f780ebcc8122268ee6f2e796e8176632b817f3874d8dc4fc405f9c4"
    );
}

#[test]
fn io_hash_known_answer() {
    let io = hash_io(
        "Decide the outcome per the interpretation.",
        "Facts: ...\nOptions:\n0) yes\n1) no\nChoose exactly one.",
        r#"{"option_index":1}"#,
    );
    assert_eq!(
        hex::encode(io),
        "e24990bd43a9d570ea938da194cb7323cb9b1df388211a48f7abaf37479d87c7"
    );
}

// --- determinism --------------------------------------------------------

#[test]
fn hashes_are_deterministic_across_runs() {
    let req = sample_request();
    let resp = sample_response();
    let a = ClaimMetadata::compute(&req, &resp);
    let b = ClaimMetadata::compute(&req, &resp);
    assert_eq!(a, b);
}

// --- sensitivity --------------------------------------------------------

#[test]
fn changing_model_string_flips_model_id_and_params() {
    let base = sample_config();
    let mut other = base.clone();
    other.model_id = "claude-opus-4-9".to_string();

    assert_ne!(
        hash_model_id(&base.model_id),
        hash_model_id(&other.model_id),
        "model string must flip model_id"
    );
    assert_ne!(
        hash_params(&CanonicalParams::from_config(&base)),
        hash_params(&CanonicalParams::from_config(&other)),
        "model string must flip params_hash"
    );
}

#[test]
fn changing_max_tokens_flips_params() {
    let base = sample_config();
    let mut other = base.clone();
    other.max_tokens = 2048;
    assert_ne!(
        hash_params(&CanonicalParams::from_config(&base)),
        hash_params(&CanonicalParams::from_config(&other)),
    );
}

#[test]
fn changing_thinking_flips_params() {
    let base = sample_config();
    let mut none = base.clone();
    none.thinking = None;
    let mut other = base.clone();
    other.thinking = Some("extended".to_string());
    let h_base = hash_params(&CanonicalParams::from_config(&base));
    let h_none = hash_params(&CanonicalParams::from_config(&none));
    let h_other = hash_params(&CanonicalParams::from_config(&other));
    assert_ne!(h_base, h_none);
    assert_ne!(h_base, h_other);
    assert_ne!(h_none, h_other);
}

#[test]
fn changing_provider_flips_params() {
    let base = sample_config();
    let mut other = base.clone();
    other.provider = "mock".to_string();
    assert_ne!(
        hash_params(&CanonicalParams::from_config(&base)),
        hash_params(&CanonicalParams::from_config(&other)),
    );
}

#[test]
fn changing_schema_version_flips_params() {
    let config = sample_config();
    let base = CanonicalParams::from_config(&config);
    let mut other = base;
    other.output_schema_version = base.output_schema_version + 1;
    assert_ne!(hash_params(&base), hash_params(&other));
}

#[test]
fn changing_schema_id_flips_params() {
    let config = sample_config();
    let base = CanonicalParams::from_config(&config);
    let mut other = base;
    other.output_schema_id = "kassandra.something_else";
    assert_ne!(hash_params(&base), hash_params(&other));
}

#[test]
fn changing_assembly_version_flips_params() {
    let config = sample_config();
    let base = CanonicalParams::from_config(&config);
    let mut other = base;
    other.prompt_assembly_version = base.prompt_assembly_version + 1;
    assert_ne!(hash_params(&base), hash_params(&other));
}

#[test]
fn changing_input_or_response_flips_io() {
    let base = hash_io("sys", "usr", "resp");
    assert_ne!(base, hash_io("SYS", "usr", "resp"), "system flips io_hash");
    assert_ne!(base, hash_io("sys", "USR", "resp"), "user flips io_hash");
    assert_ne!(
        base,
        hash_io("sys", "usr", "RESP"),
        "raw_response flips io_hash"
    );
}

#[test]
fn changing_only_option_does_not_change_any_hash() {
    let req = sample_request();
    let resp = sample_response();
    let meta = ClaimMetadata::compute(&req, &resp);
    // The option is a plaintext payload byte; the three hashes are
    // independent of it.
    let p0 = meta.to_payload(0);
    let p1 = meta.to_payload(7);
    assert_eq!(
        &p0[..96],
        &p1[..96],
        "only the trailing option byte differs"
    );
    assert_eq!(p0[96], 0);
    assert_eq!(p1[96], 7);
}

// --- collision resistance of the joins ----------------------------------

#[test]
fn io_join_is_unambiguous() {
    // "a"+"bc" must not collide with "ab"+"c".
    assert_ne!(
        hash_io("a", "bc", "x"),
        hash_io("ab", "c", "x"),
        "length-prefixed system/user join must be collision-free"
    );
    // The system/user boundary must also be distinct from the response
    // boundary: moving bytes from user into raw_response changes the hash.
    assert_ne!(hash_io("s", "uv", ""), hash_io("s", "u", "v"));
}

#[test]
fn params_join_is_unambiguous() {
    // Moving a character across the provider/model boundary must flip the
    // hash even though the naive concatenation "anthropic"+"claude" is the
    // same total string.
    let config = sample_config();
    let a = CanonicalParams {
        provider: "anthropicX",
        model_id: "claude-opus-4-8",
        ..CanonicalParams::from_config(&config)
    };
    let b = CanonicalParams {
        provider: "anthropic",
        model_id: "Xclaude-opus-4-8",
        ..CanonicalParams::from_config(&config)
    };
    assert_ne!(hash_params(&a), hash_params(&b));
}

// --- width / payload layout (tied to R0 constants) ----------------------

#[test]
fn hashes_are_32_bytes() {
    let meta = ClaimMetadata::compute(&sample_request(), &sample_response());
    assert_eq!(meta.model_id.len(), MODEL_ID_LEN);
    assert_eq!(meta.params_hash.len(), PARAMS_HASH_LEN);
    assert_eq!(meta.io_hash.len(), IO_HASH_LEN);
    assert_eq!(MODEL_ID_LEN, 32);
}

#[test]
fn payload_has_hashes_at_expected_offsets() {
    let meta = ClaimMetadata::compute(&sample_request(), &sample_response());
    let payload = meta.to_payload(3);
    assert_eq!(payload.len(), SUBMIT_AI_CLAIM_PAYLOAD_LEN);
    assert_eq!(payload.len(), 97);
    assert_eq!(&payload[0..32], &meta.model_id);
    assert_eq!(&payload[32..64], &meta.params_hash);
    assert_eq!(&payload[64..96], &meta.io_hash);
    assert_eq!(payload[96], 3);
}
