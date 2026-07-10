//! On-chain read helpers: fetch + validate + `Pod`-decode the `Oracle`, its
//! companion `oracle_meta`, and its agreed `Fact` set through the shared
//! `kassandra_oracles_sdk::accounts` structs.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use std::str::FromStr;

use kassandra_oracles_sdk::accounts::{AccountType, Fact, Oracle};

use super::client::{JsonRpc, RpcError};

/// Byte offset of the `oracle` field inside a `Fact` account — the
/// `getProgramAccounts` `memcmp` anchor used to enumerate an oracle's facts.
/// Tied to the shared struct so a layout change breaks the build.
pub const FACT_ORACLE_OFFSET: usize = core::mem::offset_of!(Fact, oracle);
const _: () = assert!(FACT_ORACLE_OFFSET == 8);

/// A decoded RPC account: the raw data bytes + the owner program pubkey.
struct RawAccount {
    data: Vec<u8>,
    owner: [u8; 32],
}

/// Decode a `{ data: [base64, "base64"], owner: "<base58>" }` account JSON
/// object into raw bytes + owner pubkey.
fn parse_account(method: &str, account: &Value) -> Result<RawAccount, RpcError> {
    let malformed = |detail: &str| RpcError::Malformed {
        method: method.to_string(),
        detail: detail.to_string(),
    };

    let data_arr = account
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("account.data is not an array"))?;
    let b64 = data_arr
        .first()
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("account.data[0] is not a string"))?;
    // Guard the encoding tag: we always request base64.
    if data_arr.get(1).and_then(Value::as_str) != Some("base64") {
        return Err(malformed("account.data encoding is not base64"));
    }
    let data = BASE64
        .decode(b64)
        .map_err(|e| malformed(&format!("account.data base64 decode failed: {e}")))?;

    let owner_str = account
        .get("owner")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("account.owner is not a string"))?;
    let owner = decode_pubkey(owner_str)
        .map_err(|e| malformed(&format!("account.owner is not a valid pubkey: {e}")))?;

    Ok(RawAccount { data, owner })
}

/// Base58-decode a 32-byte pubkey string.
pub(super) fn decode_pubkey(s: &str) -> Result<[u8; 32], String> {
    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| format!("base58 decode failed: {e}"))?;
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| format!("expected 32 bytes, got {}", v.len()))
}

/// Validate `raw` as an account of `expected_type` (owner + tag + length),
/// returning the trimmed byte slice to decode from.
fn validate<'a>(
    pubkey: &str,
    raw: &'a RawAccount,
    expected_type: AccountType,
    type_name: &'static str,
    needed: usize,
) -> Result<&'a [u8], RpcError> {
    if raw.owner != kassandra_oracles_sdk::PROGRAM_ID.to_bytes() {
        return Err(RpcError::WrongOwner {
            pubkey: pubkey.to_string(),
            owner: bs58::encode(raw.owner).into_string(),
            expected: kassandra_oracles_sdk::PROGRAM_ID.to_string(),
        });
    }
    if raw.data.len() < needed {
        return Err(RpcError::ShortData {
            pubkey: pubkey.to_string(),
            type_name,
            needed,
            actual: raw.data.len(),
        });
    }
    let tag = raw.data[0];
    if tag != expected_type.as_u8() {
        return Err(RpcError::WrongAccountType {
            pubkey: pubkey.to_string(),
            expected: expected_type.as_u8(),
            expected_name: type_name,
            actual: tag,
        });
    }
    Ok(&raw.data[..needed])
}

/// Fetch + validate + `Pod`-decode the `Oracle` account at `oracle_pubkey`.
///
/// Verifies the account is owned by the Kassandra program and carries the
/// [`AccountType::Oracle`] tag before decoding through the shared
/// `kassandra_oracles_sdk::accounts::Oracle` struct.
pub async fn fetch_oracle(rpc: &dyn JsonRpc, oracle_pubkey: &str) -> Result<Oracle, RpcError> {
    let params = json!([
        oracle_pubkey,
        { "encoding": "base64", "commitment": "confirmed" }
    ]);
    let result = rpc.call("getAccountInfo", params).await?;
    let value = result.get("value").ok_or_else(|| RpcError::Malformed {
        method: "getAccountInfo".to_string(),
        detail: "response `result` had no `value`".to_string(),
    })?;
    if value.is_null() {
        return Err(RpcError::AccountNotFound {
            pubkey: oracle_pubkey.to_string(),
        });
    }
    let raw = parse_account("getAccountInfo", value)?;
    let bytes = validate(
        oracle_pubkey,
        &raw,
        AccountType::Oracle,
        "Oracle",
        Oracle::LEN,
    )?;
    // The SDK's `read` copies (unaligned-safe), so the RPC `Vec<u8>` is fine.
    kassandra_oracles_sdk::accounts::read::<Oracle>(bytes).map_err(|e| RpcError::Malformed {
        method: "getAccountInfo".to_string(),
        detail: format!("Oracle decode failed: {e}"),
    })
}

/// Fetch + validate + decode the companion `oracle_meta` account for an oracle.
///
/// Derives the `[b"oracle_meta", oracle]` PDA, reads it, verifies program
/// ownership + the [`AccountType::OracleMeta`] tag, and parses the length-prefixed
/// layout (`subject` / `options` / `uri` / `uri_hash`). This is how the runner
/// reads the interpretation source: the `uri` points at the metadata JSON and
/// `uri_hash` binds it (verified by the caller after fetching).
pub async fn fetch_oracle_meta(
    rpc: &dyn JsonRpc,
    oracle_pubkey: &str,
) -> Result<kassandra_oracles_sdk::accounts::OracleMeta, RpcError> {
    let oracle =
        solana_pubkey::Pubkey::from_str(oracle_pubkey).map_err(|e| RpcError::Malformed {
            method: "oracle_meta".to_string(),
            detail: format!("invalid oracle pubkey `{oracle_pubkey}`: {e}"),
        })?;
    let (meta_pda, _) = kassandra_oracles_sdk::pda::oracle_meta(&kassandra_oracles_sdk::PROGRAM_ID, &oracle);
    let meta_pk = meta_pda.to_string();

    let params = json!([
        meta_pk,
        { "encoding": "base64", "commitment": "confirmed" }
    ]);
    let result = rpc.call("getAccountInfo", params).await?;
    let value = result.get("value").ok_or_else(|| RpcError::Malformed {
        method: "getAccountInfo".to_string(),
        detail: "response `result` had no `value`".to_string(),
    })?;
    if value.is_null() {
        return Err(RpcError::AccountNotFound { pubkey: meta_pk });
    }
    let raw = parse_account("getAccountInfo", value)?;
    // Owner + tag + min-header-length checks (variable-length account).
    validate(&meta_pk, &raw, AccountType::OracleMeta, "OracleMeta", 34)?;
    kassandra_oracles_sdk::accounts::decode_oracle_meta(&raw.data).ok_or_else(|| RpcError::Malformed {
        method: "getAccountInfo".to_string(),
        detail: "OracleMeta decode failed".to_string(),
    })
}

/// An agreed fact read from chain: the on-chain `content_hash` commitment plus
/// the `uri` its off-chain content is served from — exactly what
/// [`crate::fetch::FactRef`] needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchedFact {
    /// The on-chain 32-byte `content_hash` commitment.
    pub content_hash: [u8; 32],
    /// The fact content `uri` (decoded from the on-chain `uri[..uri_len]`).
    pub uri: String,
}

/// Enumerate an oracle's AGREED facts via `getProgramAccounts`.
///
/// Filters to accounts of `dataSize == Fact::LEN` whose `Fact.oracle` field (at
/// [`FACT_ORACLE_OFFSET`]) equals `oracle_pubkey` (the `memcmp` `bytes` are
/// base58, the RPC default), decodes each through the shared
/// `kassandra_oracles_sdk::accounts::Fact` struct, and keeps the ones with the
/// `agreed` flag set. Facts are returned sorted by `content_hash` so the result
/// is deterministic regardless of RPC ordering (prompt assembly re-sorts too,
/// but a stable order keeps logs/tests predictable).
pub async fn fetch_agreed_facts(
    rpc: &dyn JsonRpc,
    oracle_pubkey: &str,
) -> Result<Vec<FetchedFact>, RpcError> {
    let program_id = kassandra_oracles_sdk::PROGRAM_ID.to_string();
    let params = json!([
        program_id,
        {
            "encoding": "base64",
            "commitment": "confirmed",
            "filters": [
                { "dataSize": Fact::LEN },
                { "memcmp": { "offset": FACT_ORACLE_OFFSET, "bytes": oracle_pubkey } }
            ]
        }
    ]);
    let result = rpc.call("getProgramAccounts", params).await?;
    let entries = result.as_array().ok_or_else(|| RpcError::Malformed {
        method: "getProgramAccounts".to_string(),
        detail: "result is not an array".to_string(),
    })?;

    let mut facts = Vec::new();
    for entry in entries {
        let pubkey = entry
            .get("pubkey")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>")
            .to_string();
        let account = entry.get("account").ok_or_else(|| RpcError::Malformed {
            method: "getProgramAccounts".to_string(),
            detail: "entry had no `account`".to_string(),
        })?;
        let raw = parse_account("getProgramAccounts", account)?;
        let bytes = validate(&pubkey, &raw, AccountType::Fact, "Fact", Fact::LEN)?;
        let fact =
            kassandra_oracles_sdk::accounts::read::<Fact>(bytes).map_err(|e| RpcError::Malformed {
                method: "getProgramAccounts".to_string(),
                detail: format!("Fact decode failed: {e}"),
            })?;

        if !fact.is_agreed() {
            continue;
        }
        let uri = decode_uri(&pubkey, &fact)?;
        facts.push(FetchedFact {
            content_hash: fact.content_hash,
            uri,
        });
    }

    facts.sort_by(|a, b| a.content_hash.cmp(&b.content_hash));
    Ok(facts)
}

/// Decode a `Fact`'s `uri[..uri_len]` as UTF-8.
fn decode_uri(pubkey: &str, fact: &Fact) -> Result<String, RpcError> {
    let len = fact.uri_len as usize;
    let malformed = |detail: String| RpcError::Malformed {
        method: "getProgramAccounts".to_string(),
        detail,
    };
    if len > fact.uri.len() {
        return Err(malformed(format!(
            "fact `{pubkey}` uri_len {len} exceeds the {}-byte uri field",
            fact.uri.len()
        )));
    }
    String::from_utf8(fact.uri[..len].to_vec())
        .map_err(|e| malformed(format!("fact `{pubkey}` uri is not valid UTF-8: {e}")))
}

