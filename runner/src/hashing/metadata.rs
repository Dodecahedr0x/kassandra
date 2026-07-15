//! The [`ClaimMetadata`] triple — the three canonical hashes bundled together —
//! plus its computation from a request/response pair and its serialization into
//! the on-chain `submit_ai_claim` instruction payload.

use crate::constants::{
    IO_HASH_LEN, MODEL_ID_LEN, OPTION_LEN, PARAMS_HASH_LEN, SUBMIT_AI_CLAIM_PAYLOAD_LEN,
};
use crate::provider::{CompletionRequest, CompletionResponse};

use super::{hash_io, hash_model_id, hash_params, CanonicalParams};

/// The three canonical claim hashes. `option` is deliberately NOT a field here
/// — it is a separate plaintext byte supplied to [`ClaimMetadata::to_payload`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimMetadata {
    /// `sha256(model_id_string_utf8)`.
    pub model_id: [u8; MODEL_ID_LEN],
    /// `sha256(canonical_params_bytes)`.
    pub params_hash: [u8; PARAMS_HASH_LEN],
    /// `sha256(canonical_input ++ raw_response)`.
    pub io_hash: [u8; IO_HASH_LEN],
}

impl ClaimMetadata {
    /// Compute all three hashes from the request the submitter sent and the
    /// response they received. The model string + params come from the
    /// RESOLVED values in the response (what actually answered); the I/O
    /// commitment covers the request's `system`/`user` + the verbatim
    /// `raw_response`.
    pub fn compute(req: &CompletionRequest, resp: &CompletionResponse) -> Self {
        Self {
            model_id: hash_model_id(&resp.model_id),
            params_hash: hash_params(&CanonicalParams::from_config(&resp.params)),
            io_hash: hash_io(&req.system, &req.user, &resp.raw_response),
        }
    }

    /// Assemble the exact `submit_ai_claim` instruction payload (after the
    /// 1-byte discriminant): `model_id[32] ++ params_hash[32] ++ io_hash[32] ++
    /// option[1]` = 97 bytes. Offsets and widths are tied to the R0 payload
    /// constants, not loose literals.
    pub fn to_payload(&self, option: u8) -> [u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN] {
        let mut out = [0u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN];
        let model_end = MODEL_ID_LEN;
        let params_end = model_end + PARAMS_HASH_LEN;
        let io_end = params_end + IO_HASH_LEN;
        out[..model_end].copy_from_slice(&self.model_id);
        out[model_end..params_end].copy_from_slice(&self.params_hash);
        out[params_end..io_end].copy_from_slice(&self.io_hash);
        // OPTION_LEN == 1: the option is the single trailing byte.
        debug_assert_eq!(OPTION_LEN, 1);
        out[io_end] = option;
        out
    }
}
