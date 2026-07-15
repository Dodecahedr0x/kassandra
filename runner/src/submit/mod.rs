//! Build + sign + send + confirm the `submit_ai_claim` transaction (Task RS1).
//!
//! Turns the runner into a self-contained keeper: given the 97-byte
//! `submit_ai_claim` payload the runner ALREADY computes ([`crate::hashing`]),
//! the oracle/proposer pubkeys, and the proposer-authority keypair, this module
//!
//! 1. builds the `submit_ai_claim` [`Instruction`] (exact processor account
//!    order + `data = [Ix::SubmitAiClaim] ++ payload`),
//! 2. wraps it in a legacy [`Message`] (payer = the authority) with a fetched
//!    recent blockhash and SIGNS it (ed25519) with the loaded keypair,
//! 3. serializes it (bincode → base64) and SENDS it over the same I3
//!    [`JsonRpc`] transport used for the on-chain fetch, then
//! 4. CONFIRMS it by polling `getSignatureStatuses`, surfacing a failed tx /
//!    program error (e.g. an already-submitted claim, or a wrong-phase reject)
//!    as a clear [`SubmitError`].
//!
//! # Why the split solana-* crates
//!
//! The legacy-message compaction (compact-u16 account/instruction encoding) and
//! ed25519 signing are subtle; we use the granular `solana-message` /
//! `solana-transaction` / `solana-keypair` crates for a correct, canonical
//! serialization rather than hand-rolling it — but deliberately NOT
//! `solana-client` (send/confirm rides the existing reqwest [`JsonRpc`]) nor the
//! full `solana-sdk`. These are host-only deps; the on-chain (pinocchio) program
//! is untouched.
//!
//! # Payload provenance
//!
//! The transaction carries the SAME 97 payload bytes the runner emits in its
//! `RunOutput` (`model_id[32] ++ params_hash[32] ++ io_hash[32] ++ option[1]`);
//! it is passed in, never recomputed here, so the submitted claim and the
//! emitted metadata can never diverge.
//!
//! [`Instruction`]: solana_instruction::Instruction
//! [`Message`]: solana_message::Message
//! [`JsonRpc`]: crate::rpc::JsonRpc

mod build;
mod confirm;
mod error;

pub use build::{
    build_signed_transaction, build_submit_ai_claim_ix, derive_ai_claim_pda, derive_proposer_pda,
    encode_transaction, load_keypair, program_id, SUBMIT_AI_CLAIM_DISCRIMINANT,
};
pub use confirm::{
    confirm, get_latest_blockhash, send_transaction, submit_and_confirm, ConfirmOptions,
    Confirmation,
};
pub use error::SubmitError;

#[cfg(test)]
mod tests;
