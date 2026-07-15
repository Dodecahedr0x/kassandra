//! Agreed-fact fetching + `content_hash` verification (Task R3).
//!
//! An oracle's agreed facts live on-chain as a `content_hash: [u8; 32]` plus a
//! `uri` (≤200 bytes); the fact *content* itself is off-chain at that `uri`.
//! This module fetches each `uri`, verifies the fetched bytes against the
//! on-chain `content_hash`, and — only on a match — produces the verified
//! [`Fact`] pairs that [`crate::prompt::assemble`] feeds to the model.
//!
//! # The security contract
//!
//! `content_hash` is the on-chain commitment to the exact fact bytes the oracle
//! agreed on. The model's answer (and therefore the on-chain claim) is only
//! sound if the content it reasons over is *exactly* that committed content.
//! So tampered, swapped, or unavailable content is **REJECTED with a clear
//! error — never silently fed to the model**. A mismatch is treated as a hard
//! failure, identical to a fetch failure.
//!
//! # `content_hash` derivation (mirrored from the program)
//!
//! The on-chain program (`programs/oracles/src/processor/submit_fact.rs`)
//! treats `content_hash` as an **opaque, caller-supplied 32-byte value**: it
//! uses it only as a `Fact` PDA seed (`[b"fact", oracle, content_hash]`) and
//! stores it verbatim — it never hashes the content or applies any framing
//! (the program tests submit arbitrary values such as `[0x42; 32]`). The
//! derivation is therefore a purely *off-chain* convention, defined here as the
//! reference for both proposer and challenger:
//!
//! > **`content_hash = sha256(raw fact content bytes)`** — plain SHA-256 over
//! > the exact bytes served at the `uri`, with no length prefix, no domain tag,
//! > and no other framing.
//!
//! Verification recomputes `sha256(body)` over the **raw response bytes** (not
//! the decoded string) and compares to the on-chain `content_hash`.
//!
//! # Encoding policy (non-UTF-8)
//!
//! The verified content is rendered into the prompt as text and is what
//! `content_hash` committed to, so it MUST be valid UTF-8. A body that hashes
//! correctly but is not valid UTF-8 is **rejected** ([`VerifyError::NonUtf8`])
//! rather than lossily decoded — a lossy decode would diverge from the bytes
//! the hash committed to.
//!
//! # Fetcher abstraction (offline tests)
//!
//! Fetching is behind the [`FactFetcher`] trait so the verification logic runs
//! offline in tests. [`HttpFactFetcher`] is the real `reqwest`-based default
//! (http/https only, with a timeout and a body-size cap); [`MockFactFetcher`]
//! is a deterministic, no-network map used by the tests.
//!
//! # Resource limits + SSRF (documented limitations)
//!
//! [`HttpFactFetcher`] caps the response body at [`DEFAULT_MAX_BODY_BYTES`]
//! (overridable via [`HttpFactFetcher::with_max_body_bytes`]): it rejects a
//! declared `Content-Length` over the cap up front, and streams the body
//! chunk-by-chunk, aborting with [`FetchError::TooLarge`] the moment the
//! accumulated size would exceed the cap — so an unbounded or hostile body can't
//! exhaust memory.
//!
//! **SSRF is NOT mitigated here.** The scheme allowlist (http/https) stops
//! `file:`/`data:`/etc., but a fact `uri` may still resolve to an internal /
//! link-local / loopback address (e.g. `http://169.254.169.254/...` or
//! `http://10.0.0.5/...`), and redirects are followed within `reqwest`'s default
//! cap. Treat fact URIs as untrusted: run the runner where it has no privileged
//! network position, or add egress filtering / DNS-pinning at the deployment
//! layer. This is a deliberate, documented limitation for v1.
//!
//! # Batch policy (fail-fast)
//!
//! [`fetch_and_verify_facts`] is **fail-fast**: it returns the first
//! [`VerifyError`] (fetch failure, hash mismatch, or non-UTF-8), naming the
//! offending `uri`. One bad fact invalidates the whole agreed set for a run —
//! there is no partial/best-effort fact set, so collecting further errors would
//! add no value.

mod http;
mod mock;
mod types;

#[cfg(test)]
mod tests;

pub use http::*;
pub use mock::*;
pub use types::*;
