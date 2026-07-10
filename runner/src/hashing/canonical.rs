//! Canonical preimage construction and the three per-input claim hashes.
//!
//! This module owns the byte-exact encoding primitives (length-prefixed
//! strings, fixed-width big-endian integers, the `sha256` helper) and the
//! [`CanonicalParams`] preimage plus the `hash_model_id` / `hash_params` /
//! `hash_io` functions built on them.

use sha2::{Digest, Sha256};

use crate::constants::{IO_HASH_LEN, MODEL_ID_LEN, PARAMS_HASH_LEN};
use crate::provider::ModelConfig;

/// Version of the runner's prompt-assembly contract (Task R2). It is folded
/// into `params_hash` so that if the assembly of `system`/`user` ever changes,
/// claims produced by different assembly versions hash differently. **Bump this
/// whenever R2's prompt assembly changes in a way that affects the model
/// input.**
pub const PROMPT_ASSEMBLY_VERSION: u32 = 1;

/// Stable identifier of the structured-output schema the runner forces the
/// model to answer in (the categorical `{ "option_index": <int> }` shape). Part
/// of `params_hash` so a different answer schema hashes differently.
pub const OUTPUT_SCHEMA_ID: &str = "kassandra.categorical_option_index";

/// Version of [`OUTPUT_SCHEMA_ID`]. Bump when the schema's shape changes.
pub const OUTPUT_SCHEMA_VERSION: u32 = 1;

/// Append a string as `u32be(len) ++ utf8_bytes`.
///
/// The 4-byte big-endian length prefix makes every string field
/// self-delimiting, so two adjacent strings can never be confused with a
/// different split of the same concatenation. (Strings longer than `u32::MAX`
/// bytes — ~4 GiB — are out of scope; the cast would wrap, which the protocol
/// does not support.)
fn put_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u32).to_be_bytes());
    buf.extend_from_slice(s.as_bytes());
}

/// Append an `Option<&str>` as a 1-byte presence tag (`0x00` = none, `0x01` =
/// some) followed, when present, by the length-prefixed string.
fn put_opt_str(buf: &mut Vec<u8>, s: Option<&str>) {
    match s {
        None => buf.push(0u8),
        Some(s) => {
            buf.push(1u8);
            put_str(buf, s);
        }
    }
}

/// `sha256` over `bytes`. The return type is tied to the R0 payload width
/// constant: this only compiles while `MODEL_ID_LEN == 32`, so a drift in the
/// pinned width breaks the build rather than silently producing a wrong hash.
fn sha256(bytes: &[u8]) -> [u8; MODEL_ID_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// The deterministic preimage for `params_hash`: every config input that
/// affects the categorical answer, in a fixed field order.
///
/// Use [`CanonicalParams::from_config`] to build one from a resolved
/// [`ModelConfig`] with the runner's own schema/assembly version constants
/// filled in. The struct is exposed (and the versions are public fields) so a
/// challenger — or a sensitivity test — can construct any variant explicitly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalParams<'a> {
    /// The runner's prompt-assembly version ([`PROMPT_ASSEMBLY_VERSION`]).
    pub prompt_assembly_version: u32,
    /// Provider identifier (e.g. `"anthropic"`).
    pub provider: &'a str,
    /// Resolved model identifier string (e.g. `"claude-opus-4-8"`).
    pub model_id: &'a str,
    /// Thinking mode declared to the provider (e.g. `Some("adaptive")`).
    pub thinking: Option<&'a str>,
    /// Output-schema identifier ([`OUTPUT_SCHEMA_ID`]).
    pub output_schema_id: &'a str,
    /// Output-schema version ([`OUTPUT_SCHEMA_VERSION`]).
    pub output_schema_version: u32,
    /// Upper bound on generated tokens.
    pub max_tokens: u32,
}

impl<'a> CanonicalParams<'a> {
    /// Build the canonical params from a resolved [`ModelConfig`], filling the
    /// output-schema id/version and prompt-assembly version from the runner's
    /// own constants. This is the production path; both the proposer and a
    /// challenger running the same runner version produce identical bytes.
    pub fn from_config(config: &'a ModelConfig) -> Self {
        Self {
            prompt_assembly_version: PROMPT_ASSEMBLY_VERSION,
            provider: &config.provider,
            model_id: &config.model_id,
            thinking: config.thinking.as_deref(),
            output_schema_id: OUTPUT_SCHEMA_ID,
            output_schema_version: OUTPUT_SCHEMA_VERSION,
            max_tokens: config.max_tokens,
        }
    }

    /// The exact canonical preimage bytes hashed into `params_hash`.
    ///
    /// Fixed field order (THIS ordering IS the spec — never reorder):
    /// 1. `prompt_assembly_version` — `u32` big-endian
    /// 2. `provider`               — `u32be(len) ++ utf8`
    /// 3. `model_id`               — `u32be(len) ++ utf8`
    /// 4. `thinking`               — `0x00` | `0x01 ++ u32be(len) ++ utf8`
    /// 5. `output_schema_id`       — `u32be(len) ++ utf8`
    /// 6. `output_schema_version`  — `u32` big-endian
    /// 7. `max_tokens`             — `u32` big-endian
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.prompt_assembly_version.to_be_bytes());
        put_str(&mut buf, self.provider);
        put_str(&mut buf, self.model_id);
        put_opt_str(&mut buf, self.thinking);
        put_str(&mut buf, self.output_schema_id);
        buf.extend_from_slice(&self.output_schema_version.to_be_bytes());
        buf.extend_from_slice(&self.max_tokens.to_be_bytes());
        buf
    }
}

/// `model_id = sha256(model_id_string_utf8)`.
///
/// The preimage is the verbatim UTF-8 bytes of the resolved model identifier
/// string — nothing else (no length prefix, no separators).
pub fn hash_model_id(model_id_string: &str) -> [u8; MODEL_ID_LEN] {
    sha256(model_id_string.as_bytes())
}

/// `params_hash = sha256(params.to_canonical_bytes())`.
///
/// See [`CanonicalParams::to_canonical_bytes`] for the exact preimage layout.
pub fn hash_params(params: &CanonicalParams) -> [u8; PARAMS_HASH_LEN] {
    sha256(&params.to_canonical_bytes())
}

/// `io_hash = sha256( u32be(len(system)) ++ system ++ u32be(len(user)) ++ user
/// ++ raw_response )`.
///
/// `system` and `user` are the exact assembled model input strings the
/// submitter sent; `raw_response` is the model's verbatim response text (the
/// structured-output JSON string, byte-for-byte as returned). `system` and
/// `user` are length-prefixed so the boundary between them is unambiguous;
/// `raw_response` is appended verbatim and consumes the remainder, so no
/// trailing length prefix is needed. The result commits to the EXACT
/// (input, output) pair the submitter used.
pub fn hash_io(system: &str, user: &str, raw_response: &str) -> [u8; IO_HASH_LEN] {
    let mut buf = Vec::new();
    put_str(&mut buf, system);
    put_str(&mut buf, user);
    buf.extend_from_slice(raw_response.as_bytes());
    sha256(&buf)
}
