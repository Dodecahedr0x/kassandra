//! The offline mock transport: a deterministic, no-network [`JsonRpc`]
//! implementation used by tests.

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::Value;

use super::client::{JsonRpc, RpcError};

// --- offline mock transport -------------------------------------------------

/// A deterministic, no-network [`JsonRpc`] backed by a `method -> canned result`
/// map. Used by tests (and cross-module tests) to serve canned
/// `getAccountInfo` / `getProgramAccounts` responses built from real
/// `Oracle`/`Fact` Pod byte layouts — mirrors [`crate::fetch::MockFactFetcher`].
#[derive(Clone, Debug, Default)]
pub struct MockRpc {
    responses: std::collections::HashMap<String, Value>,
}

impl MockRpc {
    /// An empty mock (every method errors as malformed/absent).
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the `result` value returned for `method` (builder-style).
    pub fn with(mut self, method: impl Into<String>, result: Value) -> Self {
        self.responses.insert(method.into(), result);
        self
    }

    /// Base64-encode raw account bytes for a canned `account.data` field.
    pub fn base64(bytes: &[u8]) -> String {
        BASE64.encode(bytes)
    }

    /// The Kassandra program id as base58 (the canned account `owner`).
    pub fn program_owner() -> String {
        kassandra_oracles_sdk::PROGRAM_ID.to_string()
    }
}

#[async_trait]
impl JsonRpc for MockRpc {
    async fn call(&self, method: &str, _params: Value) -> Result<Value, RpcError> {
        self.responses
            .get(method)
            .cloned()
            .ok_or_else(|| RpcError::Malformed {
                method: method.to_string(),
                detail: "no canned response registered".to_string(),
            })
    }
}
