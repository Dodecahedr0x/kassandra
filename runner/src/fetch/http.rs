//! The real `reqwest`-based [`HttpFactFetcher`] and its resource-limit config.

use std::time::Duration;

use async_trait::async_trait;

use crate::fetch::{FactFetcher, FetchError};

/// Default HTTP request timeout for [`HttpFactFetcher`].
pub const DEFAULT_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Default maximum fact-content body size for [`HttpFactFetcher`] (8 MiB). A
/// body exceeding this is rejected with [`FetchError::TooLarge`] so a hostile or
/// unbounded response can't exhaust memory. Fact content is small text, so this
/// is generous.
pub const DEFAULT_MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// The default `reqwest`-based fetcher: an HTTP(S) `GET` returning the raw
/// response body bytes.
///
/// **Scheme policy:** only `http` and `https` are accepted; any other scheme is
/// rejected up front with [`FetchError::UnsupportedScheme`] (so a fact `uri`
/// can never reach the local filesystem, `data:` blobs, etc.).
///
/// **Status policy:** non-2xx responses are errors ([`FetchError::Status`]).
///
/// **Timeout:** a per-request timeout (default [`DEFAULT_FETCH_TIMEOUT`]) bounds
/// each fetch. Redirects follow `reqwest`'s default policy (capped); nothing
/// exotic is enabled.
///
/// **Body-size cap:** the response body is capped at `max_body_bytes` (default
/// [`DEFAULT_MAX_BODY_BYTES`]) — a declared `Content-Length` over the cap is
/// rejected up front, and the body is streamed chunk-by-chunk and aborted with
/// [`FetchError::TooLarge`] the moment it would exceed the cap.
#[derive(Clone, Debug)]
pub struct HttpFactFetcher {
    client: reqwest::Client,
    max_body_bytes: usize,
}

impl HttpFactFetcher {
    /// Build a fetcher with the [default timeout](DEFAULT_FETCH_TIMEOUT) and
    /// [default body cap](DEFAULT_MAX_BODY_BYTES).
    pub fn new() -> Result<Self, FetchError> {
        Self::with_timeout(DEFAULT_FETCH_TIMEOUT)
    }

    /// Build a fetcher with a custom request timeout (default body cap).
    pub fn with_timeout(timeout: Duration) -> Result<Self, FetchError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| FetchError::Transport {
                uri: "<client init>".to_string(),
                message: e.to_string(),
            })?;
        Ok(Self {
            client,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
        })
    }

    /// Override the maximum response body size (builder-style).
    pub fn with_max_body_bytes(mut self, max_body_bytes: usize) -> Self {
        self.max_body_bytes = max_body_bytes;
        self
    }
}

/// The scheme of `uri` (lowercased), i.e. the text before the first `:`.
fn scheme_of(uri: &str) -> Option<String> {
    uri.split_once(':').map(|(s, _)| s.to_ascii_lowercase())
}

#[async_trait]
impl FactFetcher for HttpFactFetcher {
    async fn fetch(&self, uri: &str) -> Result<Vec<u8>, FetchError> {
        // Scheme allowlist FIRST — never hand a non-http(s) uri to reqwest.
        match scheme_of(uri).as_deref() {
            Some("http") | Some("https") => {}
            other => {
                return Err(FetchError::UnsupportedScheme {
                    uri: uri.to_string(),
                    scheme: other.unwrap_or("").to_string(),
                });
            }
        }

        let resp = self
            .client
            .get(uri)
            .send()
            .await
            .map_err(|e| FetchError::Transport {
                uri: uri.to_string(),
                message: e.to_string(),
            })?;

        let status = resp.status();
        if !status.is_success() {
            return Err(FetchError::Status {
                uri: uri.to_string(),
                status: status.as_u16(),
            });
        }

        // Reject an over-cap declared Content-Length up front (cheap, before
        // reading the body).
        if let Some(len) = resp.content_length() {
            if len > self.max_body_bytes as u64 {
                return Err(FetchError::TooLarge {
                    uri: uri.to_string(),
                    limit: self.max_body_bytes,
                });
            }
        }

        // Stream chunk-by-chunk so a body with no/lying Content-Length still
        // can't exceed the cap or exhaust memory.
        let mut resp = resp;
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| FetchError::Transport {
            uri: uri.to_string(),
            message: e.to_string(),
        })? {
            if buf.len() + chunk.len() > self.max_body_bytes {
                return Err(FetchError::TooLarge {
                    uri: uri.to_string(),
                    limit: self.max_body_bytes,
                });
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    }
}
