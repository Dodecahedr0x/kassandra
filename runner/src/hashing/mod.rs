//! Canonical claim-metadata hashing — THE off-chain protocol contract.
//!
//! The on-chain program stores three opaque 32-byte commitments in `AiClaim`
//! (`model_id`, `params_hash`, `io_hash`) plus a 1-byte categorical `option`.
//! It does NOT compute them. This module defines the canonical, byte-exact
//! scheme that a proposer's runner uses to produce them AND that a challenger's
//! independent re-run must follow to reproduce them. The full byte layout is
//! documented in `runner/HASHING.md` (and mirrored in the rustdoc below); that
//! document is the protocol spec.
//!
//! # Determinism is the whole point
//!
//! Every preimage byte is defined deterministically: there is no map iteration,
//! no float formatting, no locale, no timestamps, no platform-dependent integer
//! widths. All integers are fixed-width **big-endian**; all strings are their
//! verbatim UTF-8 bytes with an explicit **4-byte big-endian length prefix** so
//! adjacent fields can never collide (e.g. `"a"+"bc"` cannot alias `"ab"+"c"`).
//! A third party with only this spec + the same inputs reproduces identical
//! 32-byte hashes in any language.
//!
//! # The three hashes
//!
//! 1. **`model_id`** = `sha256(model_id_string_utf8)`. The model string is the
//!    resolved pinned model identifier ([`crate::provider::ModelConfig::model_id`],
//!    as echoed back in [`crate::provider::CompletionResponse::model_id`]) —
//!    e.g. `"claude-opus-4-8"`. See [`hash_model_id`].
//!
//! 2. **`params_hash`** = `sha256(canonical_params_bytes)`, a fixed-field-order
//!    length-prefixed serialization of every config input that affects the
//!    answer: the prompt-assembly version, provider id, model string, thinking
//!    mode, output-schema id + version, and `max_tokens`. See [`hash_params`]
//!    and [`CanonicalParams`].
//!
//! 3. **`io_hash`** = `sha256(len(system)‖system ‖ len(user)‖user ‖ raw_response)`
//!    — a COMMITMENT to the exact (assembled input, verbatim raw response) the
//!    submitter used. See [`hash_io`].
//!
//! Note `option` is NOT hashed: it is a separate plaintext byte in the payload.

mod canonical;
mod metadata;

#[cfg(test)]
mod tests;

pub use canonical::{
    hash_io, hash_model_id, hash_params, CanonicalParams, OUTPUT_SCHEMA_ID, OUTPUT_SCHEMA_VERSION,
    PROMPT_ASSEMBLY_VERSION,
};
pub use metadata::ClaimMetadata;
