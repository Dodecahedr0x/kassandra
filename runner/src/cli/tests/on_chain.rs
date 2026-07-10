//! On-chain config build (offline via MockRpc).

use super::sha256_hex;
use crate::cli::{build_config_from_chain, build_model_config, run_core};
use crate::fetch::MockFactFetcher;
use crate::provider::MockProvider;
use crate::rpc::MockRpc;
use sha2::{Digest, Sha256};

/// Hand-encode an `oracle_meta` account body (mirrors `write_oracle_meta`).
fn oracle_meta_account_bytes(
    oracle: [u8; 32],
    subject: &str,
    options: &[&str],
    uri: &str,
    uri_hash: [u8; 32],
) -> Vec<u8> {
    use kassandra_oracles_sdk::accounts::AccountType;
    let mut d = vec![AccountType::OracleMeta.as_u8(), 255];
    d.extend_from_slice(&oracle);
    d.extend_from_slice(&(subject.len() as u16).to_le_bytes());
    d.extend_from_slice(subject.as_bytes());
    d.push(options.len() as u8);
    for o in options {
        d.extend_from_slice(&(o.len() as u16).to_le_bytes());
        d.extend_from_slice(o.as_bytes());
    }
    d.extend_from_slice(&(uri.len() as u16).to_le_bytes());
    d.extend_from_slice(uri.as_bytes());
    d.extend_from_slice(&uri_hash);
    d
}

#[tokio::test]
async fn build_config_from_chain_reads_meta_and_runs() {
    use bytemuck::Zeroable;
    use kassandra_oracles_sdk::accounts::{AccountType, Fact};
    use serde_json::json;

    let oracle_pk = "So11111111111111111111111111111111111111112";
    let oracle_bytes: [u8; 32] = bs58::decode(oracle_pk)
        .into_vec()
        .unwrap()
        .try_into()
        .unwrap();

    // The off-chain metadata JSON that `oracle_meta.uri` points at.
    let meta_uri = "https://meta.example/oracle.json";
    let meta_json = r#"{"version":1,"subject":"Did BTC close >= $100k?","options":["Yes","No"],"promptTemplate":"Resolve YES if BTC closed at or above $100,000; otherwise NO."}"#;
    let uri_hash: [u8; 32] = Sha256::digest(meta_json.as_bytes()).into();
    let meta_bytes = oracle_meta_account_bytes(
        oracle_bytes,
        "Did BTC close >= $100k?",
        &["Yes", "No"],
        meta_uri,
        uri_hash,
    );

    // One agreed fact whose off-chain content the mock fetcher serves.
    let fact_content = b"BTC closed at $98,000.";
    let content_hash: [u8; 32] = Sha256::digest(fact_content).into();
    let fact_uri = "https://facts.example/btc";
    let mut fact = Fact::zeroed();
    fact.account_type = AccountType::Fact.as_u8();
    fact.oracle = oracle_bytes.into();
    fact.content_hash = content_hash;
    fact.uri_len = fact_uri.len() as u16;
    fact.uri[..fact_uri.len()].copy_from_slice(fact_uri.as_bytes());
    fact.agreed = 1;

    let owner = MockRpc::program_owner();
    let rpc = MockRpc::new()
        .with(
            "getAccountInfo",
            json!({
                "context": { "slot": 1 },
                "value": {
                    "data": [MockRpc::base64(&meta_bytes), "base64"],
                    "owner": owner,
                    "lamports": 1u64, "executable": false, "rentEpoch": 0u64,
                    "space": meta_bytes.len(),
                }
            }),
        )
        .with(
            "getProgramAccounts",
            json!([{
                "pubkey": "Fact111111111111111111111111111111111111111",
                "account": {
                    "data": [MockRpc::base64(bytemuck::bytes_of(&fact)), "base64"],
                    "owner": owner,
                    "lamports": 1u64, "executable": false, "rentEpoch": 0u64,
                    "space": Fact::LEN,
                }
            }]),
        );

    // The fetcher serves BOTH the metadata JSON (verified vs uri_hash) and the
    // fact content.
    let fetcher = MockFactFetcher::new()
        .with(meta_uri, meta_json.as_bytes().to_vec())
        .with(fact_uri, fact_content.to_vec());

    let config = build_config_from_chain(&rpc, &fetcher, oracle_pk)
        .await
        .unwrap();
    assert!(config.interpretation.contains("Resolve YES"));
    assert_eq!(config.options_count, 2);
    assert_eq!(config.option_labels.as_ref().unwrap().len(), 2);
    assert_eq!(config.facts.len(), 1);
    assert_eq!(config.facts[0].content_hash, sha256_hex(fact_content));
    assert_eq!(config.oracle.as_deref(), Some(oracle_pk));

    // And it drives the existing pipeline (fact fetch + mock provider).
    let provider = MockProvider::new(0, r#"{"option_index":0}"#, "mock-claude");
    let out = run_core(&config, build_model_config(None, None), &fetcher, &provider)
        .await
        .unwrap();
    assert_eq!(out.option_index, 0);
}

#[tokio::test]
async fn build_config_from_chain_rejects_json_hash_mismatch() {
    use serde_json::json;

    let oracle_pk = "So11111111111111111111111111111111111111112";
    let oracle_bytes: [u8; 32] = bs58::decode(oracle_pk)
        .into_vec()
        .unwrap()
        .try_into()
        .unwrap();
    let meta_uri = "https://meta.example/oracle.json";
    // uri_hash commits to THIS json...
    let committed = r#"{"promptTemplate":"the real rules"}"#;
    let uri_hash: [u8; 32] = Sha256::digest(committed.as_bytes()).into();
    let meta_bytes =
        oracle_meta_account_bytes(oracle_bytes, "Q?", &["Yes", "No"], meta_uri, uri_hash);

    let rpc = MockRpc::new().with(
        "getAccountInfo",
        json!({
            "context": { "slot": 1 },
            "value": {
                "data": [MockRpc::base64(&meta_bytes), "base64"],
                "owner": MockRpc::program_owner(),
                "lamports": 1u64, "executable": false, "rentEpoch": 0u64,
                "space": meta_bytes.len(),
            }
        }),
    );
    // ...but the host serves DIFFERENT json → rejected.
    let fetcher = MockFactFetcher::new()
        .with(meta_uri, br#"{"promptTemplate":"tampered rules"}"#.to_vec());

    let err = build_config_from_chain(&rpc, &fetcher, oracle_pk)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("uri_hash"), "{err}");
}
