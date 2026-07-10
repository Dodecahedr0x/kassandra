//! Core fact-reference types, error taxonomy, the [`FactFetcher`] trait, and the
//! fetch+verify functions.

use async_trait::async_trait;
use sha2::{Digest, Sha256};

use crate::prompt::Fact;

/// An agreed fact as committed on-chain: the 32-byte `content_hash` plus the
/// off-chain `uri` its content is served from. (This mirrors the on-chain
/// `Fact`'s `content_hash` + `uri`; the runner takes it as explicit input.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactRef {
    /// The on-chain commitment: `sha256(content_bytes)`.
    pub content_hash: [u8; 32],
    /// The location the fact content is fetched from (http/https).
    pub uri: String,
}

impl FactRef {
    /// Convenience constructor.
    pub fn new(content_hash: [u8; 32], uri: impl Into<String>) -> Self {
        Self {
            content_hash,
            uri: uri.into(),
        }
    }
}

/// A transport-level failure fetching a `uri`. Distinct from a *verification*
/// failure ([`VerifyError`]) — this is "couldn't get the bytes", not "the bytes
/// don't match".
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// The `uri`'s scheme is not `http`/`https`. Kept deliberately narrow:
    /// `file:`, `data:`, `ftp:`, etc. are rejected so a fact `uri` can never
    /// pull from the local filesystem or other exotic sources.
    #[error("unsupported URI scheme `{scheme}` for `{uri}` (only http/https are allowed)")]
    UnsupportedScheme {
        /// The offending uri.
        uri: String,
        /// The scheme that was rejected.
        scheme: String,
    },
    /// A transport/network error (DNS, connection, TLS, timeout, malformed
    /// URL). Carries the rendered cause so callers see why.
    #[error("fetch of `{uri}` failed: {message}")]
    Transport {
        /// The uri being fetched.
        uri: String,
        /// The rendered underlying error.
        message: String,
    },
    /// The server responded with a non-success (non-2xx) HTTP status.
    #[error("fetch of `{uri}` returned non-success HTTP status {status}")]
    Status {
        /// The uri being fetched.
        uri: String,
        /// The HTTP status code.
        status: u16,
    },
    /// The response body exceeded the configured size cap (declared
    /// `Content-Length` or streamed bytes). Rejected to bound memory.
    #[error("body of `{uri}` exceeds the {limit}-byte size cap")]
    TooLarge {
        /// The uri whose body was too large.
        uri: String,
        /// The configured cap in bytes.
        limit: usize,
    },
    /// The fetcher has no content for this `uri` (used by the mock; analogous to
    /// a 404 / DNS failure for the real fetcher).
    #[error("no content available for `{uri}`")]
    NotFound {
        /// The uri that was not found.
        uri: String,
    },
}

/// A fact failed verification against its on-chain `content_hash`. Either the
/// bytes couldn't be fetched ([`FetchError`]), they hashed to the wrong value
/// (tampered/wrong content), or they weren't valid UTF-8.
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    /// The content couldn't be fetched at all.
    #[error(transparent)]
    Fetch(#[from] FetchError),
    /// The fetched bytes did not hash to the on-chain `content_hash`: the
    /// content is tampered, swapped, or otherwise not what the oracle agreed
    /// on. The fact is REJECTED — it is never passed to the model.
    #[error(
        "content_hash mismatch for `{uri}`: expected sha256 {expected}, \
         computed {actual} (content is tampered or does not match the on-chain commitment)"
    )]
    ContentHashMismatch {
        /// The uri whose content failed verification.
        uri: String,
        /// The expected on-chain `content_hash`, hex.
        expected: String,
        /// The sha256 actually computed over the fetched body, hex.
        actual: String,
    },
    /// The bytes hashed correctly but are not valid UTF-8, so they cannot be
    /// used as prompt text. Rejected rather than lossily decoded.
    #[error("content of `{uri}` is not valid UTF-8: {message}")]
    NonUtf8 {
        /// The uri whose content was not valid UTF-8.
        uri: String,
        /// The rendered decode error.
        message: String,
    },
}

/// Fetches raw fact-content bytes for a `uri`. Behind a trait so the
/// verification logic is testable offline (see [`MockFactFetcher`]).
#[async_trait]
pub trait FactFetcher {
    /// Fetch the raw bytes served at `uri`, or a [`FetchError`] naming it.
    async fn fetch(&self, uri: &str) -> Result<Vec<u8>, FetchError>;
}

/// `sha256(bytes)` as 32 bytes — the `content_hash` derivation (plain SHA-256,
/// no framing), matching the off-chain convention the program stores opaquely.
pub(crate) fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// Fetch and verify a single agreed fact.
///
/// Fetches `fact_ref.uri`, recomputes `sha256(body)`, and:
/// - on a hash **match**, decodes the body as UTF-8 and returns the verified
///   [`Fact`];
/// - on a **mismatch**, returns [`VerifyError::ContentHashMismatch`] (the
///   content is REJECTED, never returned);
/// - on a **fetch failure**, surfaces the [`FetchError`] (via
///   [`VerifyError::Fetch`]) naming the `uri`;
/// - on **non-UTF-8** content, returns [`VerifyError::NonUtf8`].
pub async fn fetch_and_verify_fact<F>(fetcher: &F, fact_ref: &FactRef) -> Result<Fact, VerifyError>
where
    F: FactFetcher + ?Sized,
{
    let body = fetcher.fetch(&fact_ref.uri).await?;

    let actual = sha256(&body);
    if actual != fact_ref.content_hash {
        return Err(VerifyError::ContentHashMismatch {
            uri: fact_ref.uri.clone(),
            expected: hex::encode(&fact_ref.content_hash),
            actual: hex::encode(&actual),
        });
    }

    // Hash matches; decode as UTF-8 (reject non-UTF-8 rather than lossily
    // decode — the decoded text must correspond to the committed bytes).
    let content = String::from_utf8(body).map_err(|e| VerifyError::NonUtf8 {
        uri: fact_ref.uri.clone(),
        message: e.to_string(),
    })?;

    Ok(Fact {
        content_hash: fact_ref.content_hash,
        content,
    })
}

/// Fetch and verify a whole agreed-fact set, **fail-fast**.
///
/// Returns the verified [`Fact`]s (ready for [`crate::prompt::assemble`]) in the
/// SAME order as `fact_refs`, or the first [`VerifyError`] encountered. Any
/// single bad fact (unfetchable, tampered, or non-UTF-8) fails the whole run —
/// the agreed fact set is all-or-nothing.
pub async fn fetch_and_verify_facts<F>(
    fetcher: &F,
    fact_refs: &[FactRef],
) -> Result<Vec<Fact>, VerifyError>
where
    F: FactFetcher + ?Sized,
{
    let mut verified = Vec::with_capacity(fact_refs.len());
    for fact_ref in fact_refs {
        verified.push(fetch_and_verify_fact(fetcher, fact_ref).await?);
    }
    Ok(verified)
}
