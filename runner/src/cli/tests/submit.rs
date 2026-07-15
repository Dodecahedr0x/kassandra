//! `--submit` keeper mode (offline).

use super::sample_config;
use crate::cli::{
    build_model_config, resolve_submit_target, run_core, submit_claim, CommonArgs,
};
use crate::fetch::MockFactFetcher;
use crate::provider::MockProvider;
use crate::rpc::{JsonRpc, RpcError};
use crate::submit::{derive_proposer_pda, ConfirmOptions};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::json;
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::path::Path;
use std::time::Duration;

fn common_args_empty() -> CommonArgs {
    CommonArgs {
        config: None,
        oracle: None,
        rpc_url: None,
        prompt_file: None,
        mock: true,
        model: None,
        max_tokens: None,
    }
}

fn fast_confirm() -> ConfirmOptions {
    ConfirmOptions {
        max_polls: 2,
        poll_interval: Duration::from_millis(0),
        require_finalized: false,
    }
}

/// A [`JsonRpc`] that CAPTURES the base64 tx handed to `sendTransaction` so
/// the test can decode it and prove the submitted instruction data carries
/// the runner's OWN payload verbatim. Serves a canned blockhash + a
/// `confirmed` status.
struct CapturingRpc {
    sent_tx: std::sync::Mutex<Option<String>>,
    signature: String,
}

#[async_trait::async_trait]
impl JsonRpc for CapturingRpc {
    async fn call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, RpcError> {
        match method {
            "getLatestBlockhash" => Ok(json!({
                "context": { "slot": 1 },
                "value": {
                    "blockhash": Hash::new_from_array([7u8; 32]).to_string(),
                    "lastValidBlockHeight": 100
                }
            })),
            "sendTransaction" => {
                let tx = params
                    .get(0)
                    .and_then(|v| v.as_str())
                    .expect("sendTransaction param[0] is the base64 tx")
                    .to_string();
                *self.sent_tx.lock().unwrap() = Some(tx);
                Ok(json!(self.signature))
            }
            "getSignatureStatuses" => Ok(json!({
                "context": { "slot": 2 },
                "value": [ { "slot": 2, "confirmations": null, "err": null, "confirmationStatus": "confirmed" } ]
            })),
            other => Err(RpcError::Malformed {
                method: other.to_string(),
                detail: "unexpected method".to_string(),
            }),
        }
    }
}

/// Full keeper flow (fetch/config → claim → submit) against a capturing
/// MockRpc: the SUBMITTED tx must carry the RunOutput's own payload, the
/// proposer PDA must be derived from `[b"proposer", oracle, authority]`, and
/// the confirmed signature is reported.
#[tokio::test]
async fn keeper_submit_carries_runoutput_payload_and_reports_signature() {
    // Produce a real RunOutput (and its payload) via the offline pipeline.
    let content = b"BTC closed at $98,000.";
    let uri = "https://facts.example/btc";
    let config = sample_config(uri, content);
    let fetcher = MockFactFetcher::new().with(uri, content.to_vec());
    let provider = MockProvider::new(1, r#"{"option_index":1}"#, "mock-claude");
    let out = run_core(&config, build_model_config(None, None), &fetcher, &provider)
        .await
        .unwrap();

    let oracle = Pubkey::new_from_array([3u8; 32]);
    let authority = Keypair::new();
    let sig = "S".repeat(64);
    let rpc = CapturingRpc {
        sent_tx: std::sync::Mutex::new(None),
        signature: sig.clone(),
    };

    let submission = submit_claim(
        &rpc,
        &oracle,
        &authority,
        &out.submit_ai_claim_payload,
        fast_confirm(),
    )
    .await
    .unwrap();

    assert_eq!(submission.signature, sig);
    assert_eq!(submission.confirmation_status, "confirmed");
    assert_eq!(submission.oracle, oracle.to_string());
    // Proposer PDA is DERIVED from [b"proposer", oracle, authority].
    assert_eq!(
        submission.proposer,
        derive_proposer_pda(&oracle, &authority.pubkey()).to_string()
    );
    assert_eq!(submission.authority, authority.pubkey().to_string());

    // Decode the ACTUAL submitted tx: instruction data == [disc=3] ++ the
    // RunOutput payload (the reuse guarantee, end-to-end).
    let b64 = rpc.sent_tx.lock().unwrap().clone().unwrap();
    let bytes = BASE64.decode(&b64).unwrap();
    let tx: Transaction = bincode::deserialize(&bytes).unwrap();
    let ix = &tx.message.instructions[0];
    assert_eq!(ix.data[0], 3);
    assert_eq!(&ix.data[1..], &out.submit_ai_claim_payload[..]);
}

#[test]
fn submit_off_yields_no_target() {
    let common = common_args_empty();
    let config = sample_config("https://x/y", b"z");
    assert!(resolve_submit_target(&common, false, None, &config)
        .unwrap()
        .is_none());
}

#[test]
fn submit_requires_keypair() {
    let common = common_args_empty();
    let config = sample_config("https://x/y", b"z");
    let err = resolve_submit_target(&common, true, None, &config).unwrap_err();
    assert!(err.to_string().contains("--keypair"), "{err}");
}

#[test]
fn submit_explicit_mode_requires_rpc_url() {
    // Explicit-config mode (no --oracle / --rpc-url), keypair provided → the
    // missing --rpc-url must be surfaced.
    let common = common_args_empty();
    let config = sample_config("https://x/y", b"z");
    let err = resolve_submit_target(&common, true, Some(Path::new("/tmp/kp.json")), &config)
        .unwrap_err();
    assert!(err.to_string().contains("--rpc-url"), "{err}");
}

#[test]
fn submit_needs_an_oracle() {
    let mut common = common_args_empty();
    common.rpc_url = Some("http://localhost:8899".to_string());
    let config = sample_config("https://x/y", b"z"); // config.oracle == None
    let err = resolve_submit_target(&common, true, Some(Path::new("/tmp/kp.json")), &config)
        .unwrap_err();
    assert!(err.to_string().contains("oracle"), "{err}");
}

#[test]
fn submit_oracle_resolved_from_config() {
    let mut common = common_args_empty();
    common.rpc_url = Some("http://localhost:8899".to_string());
    let mut config = sample_config("https://x/y", b"z");
    let oracle_pk = "So11111111111111111111111111111111111111112";
    config.oracle = Some(oracle_pk.to_string());
    let target = resolve_submit_target(&common, true, Some(Path::new("/tmp/kp.json")), &config)
        .unwrap()
        .unwrap();
    assert_eq!(target.oracle.to_string(), oracle_pk);
    assert_eq!(target.rpc_url, "http://localhost:8899");
}
