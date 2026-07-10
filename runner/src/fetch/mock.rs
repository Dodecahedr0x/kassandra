//! The deterministic, no-network [`MockFactFetcher`] used by the tests.

use async_trait::async_trait;

use crate::fetch::{FactFetcher, FetchError};

/// A deterministic, no-network fetcher backed by a `uri -> bytes` map. Used by
/// tests: a registered `uri` returns its bytes; an unregistered one returns
/// [`FetchError::NotFound`].
#[derive(Clone, Debug, Default)]
pub struct MockFactFetcher {
    responses: std::collections::HashMap<String, Vec<u8>>,
}

impl MockFactFetcher {
    /// An empty fetcher (every `uri` is `NotFound`).
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the bytes returned for `uri` (builder-style).
    pub fn with(mut self, uri: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        self.responses.insert(uri.into(), body.into());
        self
    }

    /// Register the bytes returned for `uri` (mutating).
    pub fn insert(&mut self, uri: impl Into<String>, body: impl Into<Vec<u8>>) {
        self.responses.insert(uri.into(), body.into());
    }
}

#[async_trait]
impl FactFetcher for MockFactFetcher {
    async fn fetch(&self, uri: &str) -> Result<Vec<u8>, FetchError> {
        self.responses
            .get(uri)
            .cloned()
            .ok_or_else(|| FetchError::NotFound {
                uri: uri.to_string(),
            })
    }
}
