//! The JSON-RPC transport: the [`JsonRpc`] trait, its [`RpcError`] failure
//! type, and the real `reqwest`-backed [`HttpJsonRpc`] client.

use async_trait::async_trait;
use serde_json::{json, Value};

/// A transport- or protocol-level failure talking to the RPC endpoint, or an
/// account that failed validation before decode.
#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    /// A transport/network error (DNS, connection, TLS, timeout).
    #[error("RPC request to `{url}` failed: {message}")]
    Transport {
        /// The RPC url.
        url: String,
        /// The rendered underlying error.
        message: String,
    },
    /// A non-success HTTP status from the RPC endpoint.
    #[error("RPC `{url}` returned non-success HTTP status {status}")]
    HttpStatus {
        /// The RPC url.
        url: String,
        /// The HTTP status code.
        status: u16,
    },
    /// The JSON-RPC envelope carried an `error` object.
    #[error("RPC method `{method}` returned error {code}: {message}")]
    JsonRpc {
        /// The RPC method that failed.
        method: String,
        /// The JSON-RPC error code.
        code: i64,
        /// The JSON-RPC error message.
        message: String,
    },
    /// The RPC response JSON didn't have the expected shape.
    #[error("malformed RPC response for `{method}`: {detail}")]
    Malformed {
        /// The RPC method whose response was malformed.
        method: String,
        /// What was wrong.
        detail: String,
    },
    /// `getAccountInfo` returned `null` — the account doesn't exist.
    #[error("account `{pubkey}` not found on chain")]
    AccountNotFound {
        /// The queried pubkey (base58).
        pubkey: String,
    },
    /// The account is not owned by the Kassandra program.
    #[error("account `{pubkey}` is owned by `{owner}`, not the Kassandra program `{expected}`")]
    WrongOwner {
        /// The queried pubkey (base58).
        pubkey: String,
        /// The account's actual owner (base58).
        owner: String,
        /// The expected owner (the program id, base58).
        expected: String,
    },
    /// The account's [`AccountType`] tag byte did not match the expected type.
    #[error(
        "account `{pubkey}` has account_type tag {actual}, expected {expected} ({expected_name})"
    )]
    WrongAccountType {
        /// The queried pubkey (base58).
        pubkey: String,
        /// The expected tag byte.
        expected: u8,
        /// A human name for the expected type.
        expected_name: &'static str,
        /// The actual tag byte found.
        actual: u8,
    },
    /// The account data was shorter than the decoded struct requires.
    #[error("account `{pubkey}` data is {actual} bytes, need at least {needed} for {type_name}")]
    ShortData {
        /// The queried pubkey (base58).
        pubkey: String,
        /// The struct that was being decoded.
        type_name: &'static str,
        /// Bytes required.
        needed: usize,
        /// Bytes present.
        actual: usize,
    },
}

/// A minimal Solana JSON-RPC transport: one `call(method, params) -> result`.
///
/// Behind a trait so the whole account-decode path runs OFFLINE in tests via
/// [`MockRpc`]. [`HttpJsonRpc`] is the real `reqwest`-backed default.
#[async_trait]
pub trait JsonRpc {
    /// Invoke a JSON-RPC `method` with `params`, returning the `result` value
    /// (or an [`RpcError`] for transport / HTTP / JSON-RPC-error failures).
    async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError>;
}

/// The real `reqwest`-based JSON-RPC client (POSTs the standard
/// `{jsonrpc, id, method, params}` envelope).
#[derive(Clone, Debug)]
pub struct HttpJsonRpc {
    client: reqwest::Client,
    url: String,
}

impl HttpJsonRpc {
    /// Build a client for `url` with the fact fetcher's default timeout.
    pub fn new(url: impl Into<String>) -> Result<Self, RpcError> {
        let url = url.into();
        let client = reqwest::Client::builder()
            .timeout(crate::fetch::DEFAULT_FETCH_TIMEOUT)
            .build()
            .map_err(|e| RpcError::Transport {
                url: url.clone(),
                message: e.to_string(),
            })?;
        Ok(Self { client, url })
    }
}

#[async_trait]
impl JsonRpc for HttpJsonRpc {
    async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let resp = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| RpcError::Transport {
                url: self.url.clone(),
                message: e.to_string(),
            })?;

        let status = resp.status();
        if !status.is_success() {
            return Err(RpcError::HttpStatus {
                url: self.url.clone(),
                status: status.as_u16(),
            });
        }

        let value: Value = resp.json().await.map_err(|e| RpcError::Transport {
            url: self.url.clone(),
            message: e.to_string(),
        })?;

        if let Some(err) = value.get("error") {
            let code = err.get("code").and_then(Value::as_i64).unwrap_or(0);
            let message = err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("<no message>")
                .to_string();
            return Err(RpcError::JsonRpc {
                method: method.to_string(),
                code,
                message,
            });
        }

        value
            .get("result")
            .cloned()
            .ok_or_else(|| RpcError::Malformed {
                method: method.to_string(),
                detail: "response had neither `result` nor `error`".to_string(),
            })
    }
}
