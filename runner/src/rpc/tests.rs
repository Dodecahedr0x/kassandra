use crate::rpc::fetch::decode_pubkey;
use crate::rpc::{fetch_agreed_facts, fetch_oracle, FetchedFact, MockRpc, RpcError};
use bytemuck::Zeroable;
use kassandra_oracles_sdk::accounts::{AccountType, Fact, Oracle};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Unwrap the error side of a `fetch_oracle` result (Oracle isn't `Debug`,
/// so `unwrap_err` can't be used directly).
fn expect_oracle_err(result: Result<Oracle, RpcError>) -> RpcError {
    match result {
        Ok(_) => panic!("expected an error, got a decoded Oracle"),
        Err(e) => e,
    }
}

/// Build a canned `getAccountInfo` result wrapping `data` owned by `owner`.
fn account_info_result(data: &[u8], owner: &str) -> Value {
    json!({
        "context": { "slot": 1 },
        "value": {
            "data": [MockRpc::base64(data), "base64"],
            "owner": owner,
            "lamports": 1_000_000u64,
            "executable": false,
            "rentEpoch": 0u64,
            "space": data.len(),
        }
    })
}

/// Build a canned `getProgramAccounts` result: one entry per (pubkey, data).
fn program_accounts_result(accounts: &[(&str, Vec<u8>)], owner: &str) -> Value {
    Value::Array(
        accounts
            .iter()
            .map(|(pk, data)| {
                json!({
                    "pubkey": pk,
                    "account": {
                        "data": [MockRpc::base64(data), "base64"],
                        "owner": owner,
                        "lamports": 1_000_000u64,
                        "executable": false,
                        "rentEpoch": 0u64,
                        "space": data.len(),
                    }
                })
            })
            .collect(),
    )
}

fn sample_oracle() -> Oracle {
    let mut o = Oracle::zeroed();
    o.account_type = AccountType::Oracle.as_u8();
    o.options_count = 3;
    o.deadline = 1_900_000_000;
    o
}

fn sample_fact(oracle: [u8; 32], content_hash: [u8; 32], uri: &str, agreed: bool) -> Fact {
    let mut f = Fact::zeroed();
    f.account_type = AccountType::Fact.as_u8();
    f.oracle = oracle.into();
    f.content_hash = content_hash;
    f.uri_len = uri.len() as u16;
    f.uri[..uri.len()].copy_from_slice(uri.as_bytes());
    f.agreed = u8::from(agreed);
    f
}

const ORACLE_PK: &str = "So11111111111111111111111111111111111111112";

// --- oracle decode ------------------------------------------------------

#[tokio::test]
async fn fetch_oracle_decodes_shared_pod_fields() {
    let oracle = sample_oracle();
    let rpc = MockRpc::new().with(
        "getAccountInfo",
        account_info_result(bytemuck::bytes_of(&oracle), &MockRpc::program_owner()),
    );

    let got = fetch_oracle(&rpc, ORACLE_PK).await.unwrap();
    assert_eq!(got.options_count, 3);
    assert_eq!(got.deadline, 1_900_000_000);
}

#[tokio::test]
async fn fetch_oracle_rejects_wrong_owner() {
    let oracle = sample_oracle();
    // Owned by some other program.
    let rpc = MockRpc::new().with(
        "getAccountInfo",
        account_info_result(bytemuck::bytes_of(&oracle), ORACLE_PK),
    );
    let err = expect_oracle_err(fetch_oracle(&rpc, ORACLE_PK).await);
    assert!(matches!(err, RpcError::WrongOwner { .. }), "{err}");
}

#[tokio::test]
async fn fetch_oracle_rejects_wrong_account_type() {
    // A Fact-tagged blob padded to Oracle::LEN, owned by the program.
    let mut data = vec![0u8; Oracle::LEN];
    data[0] = AccountType::Fact.as_u8();
    let rpc = MockRpc::new().with(
        "getAccountInfo",
        account_info_result(&data, &MockRpc::program_owner()),
    );
    let err = expect_oracle_err(fetch_oracle(&rpc, ORACLE_PK).await);
    assert!(matches!(err, RpcError::WrongAccountType { .. }), "{err}");
}

#[tokio::test]
async fn fetch_oracle_reports_not_found() {
    let rpc = MockRpc::new().with(
        "getAccountInfo",
        json!({ "context": { "slot": 1 }, "value": Value::Null }),
    );
    let err = expect_oracle_err(fetch_oracle(&rpc, ORACLE_PK).await);
    assert!(matches!(err, RpcError::AccountNotFound { .. }), "{err}");
}

// --- fact enumeration ---------------------------------------------------

#[tokio::test]
async fn fetch_agreed_facts_decodes_and_filters() {
    let oracle_bytes = decode_pubkey(ORACLE_PK).unwrap();
    let ch_a = sha256(b"fact A");
    let ch_b = sha256(b"fact B");
    let ch_c = sha256(b"not agreed");
    let agreed_a = sample_fact(oracle_bytes, ch_a, "https://f/a", true);
    let agreed_b = sample_fact(oracle_bytes, ch_b, "https://f/b", true);
    let not_agreed = sample_fact(oracle_bytes, ch_c, "https://f/c", false);

    let rpc = MockRpc::new().with(
        "getProgramAccounts",
        program_accounts_result(
            &[
                (
                    "Fact111111111111111111111111111111111111111",
                    bytemuck::bytes_of(&agreed_a).to_vec(),
                ),
                (
                    "Fact222222222222222222222222222222222222222",
                    bytemuck::bytes_of(&not_agreed).to_vec(),
                ),
                (
                    "Fact333333333333333333333333333333333333333",
                    bytemuck::bytes_of(&agreed_b).to_vec(),
                ),
            ],
            &MockRpc::program_owner(),
        ),
    );

    let facts = fetch_agreed_facts(&rpc, ORACLE_PK).await.unwrap();
    // Only the two agreed facts, sorted by content_hash.
    assert_eq!(facts.len(), 2);
    let mut expected = vec![
        FetchedFact {
            content_hash: ch_a,
            uri: "https://f/a".to_string(),
        },
        FetchedFact {
            content_hash: ch_b,
            uri: "https://f/b".to_string(),
        },
    ];
    expected.sort_by(|a, b| a.content_hash.cmp(&b.content_hash));
    assert_eq!(facts, expected);
}

#[tokio::test]
async fn fetch_agreed_facts_empty_when_none() {
    let rpc = MockRpc::new().with("getProgramAccounts", Value::Array(vec![]));
    let facts = fetch_agreed_facts(&rpc, ORACLE_PK).await.unwrap();
    assert!(facts.is_empty());
}
