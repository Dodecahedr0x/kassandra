//! The send + confirm half: blockhash fetch, `sendTransaction`,
//! `getSignatureStatuses` polling, and the top-level [`submit_and_confirm`].

use std::str::FromStr;
use std::time::Duration;

use serde_json::{json, Value};
use solana_hash::Hash;
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;

use crate::constants::SUBMIT_AI_CLAIM_PAYLOAD_LEN;
use crate::rpc::JsonRpc;
use crate::submit::build::{build_signed_transaction, encode_transaction};
use crate::submit::error::SubmitError;

/// Fetch a recent blockhash via `getLatestBlockhash` (base58 → [`Hash`]).
pub async fn get_latest_blockhash(rpc: &dyn JsonRpc) -> Result<Hash, SubmitError> {
    let result = rpc
        .call("getLatestBlockhash", json!([{ "commitment": "confirmed" }]))
        .await?;
    let blockhash = result
        .get("value")
        .and_then(|v| v.get("blockhash"))
        .and_then(Value::as_str)
        .ok_or_else(|| SubmitError::Malformed {
            method: "getLatestBlockhash".to_string(),
            detail: "response had no `value.blockhash` string".to_string(),
        })?;
    Hash::from_str(blockhash).map_err(|e| SubmitError::Malformed {
        method: "getLatestBlockhash".to_string(),
        detail: format!("`value.blockhash` is not a valid base58 hash: {e}"),
    })
}

/// Send a base64-encoded, signed transaction via `sendTransaction`
/// (`encoding: base64`), returning the signature (base58).
///
/// A PREFLIGHT failure (e.g. the ai_claim PDA already exists = already-submitted,
/// or a wrong-phase reject) comes back from the RPC as a JSON-RPC error and is
/// surfaced as [`SubmitError::Rpc`].
pub async fn send_transaction(rpc: &dyn JsonRpc, tx_base64: &str) -> Result<String, SubmitError> {
    let result = rpc
        .call(
            "sendTransaction",
            json!([tx_base64, { "encoding": "base64" }]),
        )
        .await?;
    result
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| SubmitError::Malformed {
            method: "sendTransaction".to_string(),
            detail: "result was not a signature string".to_string(),
        })
}

/// How the send+confirm loop polls `getSignatureStatuses`.
#[derive(Clone, Copy, Debug)]
pub struct ConfirmOptions {
    /// Max number of status polls before giving up.
    pub max_polls: u32,
    /// Delay between polls.
    pub poll_interval: Duration,
    /// Whether `finalized` is required (else `confirmed` or `finalized` accepts).
    pub require_finalized: bool,
}

impl Default for ConfirmOptions {
    fn default() -> Self {
        // ~30s budget at 2s spacing — comfortably longer than a confirmed slot.
        Self {
            max_polls: 15,
            poll_interval: Duration::from_secs(2),
            require_finalized: false,
        }
    }
}

/// The outcome of a confirmed submission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirmation {
    /// The transaction signature (base58).
    pub signature: String,
    /// The reached confirmation status (`confirmed` / `finalized`).
    pub confirmation_status: String,
}

/// Poll `getSignatureStatuses` for `signature` until it reaches the target
/// commitment or the poll budget is exhausted.
///
/// A non-null `err` in the status is a FAILED tx → [`SubmitError::TxFailed`]. A
/// `null` status (not yet seen) or a below-target `processed` status keeps
/// polling; the budget exhausting → [`SubmitError::ConfirmTimeout`].
pub async fn confirm(
    rpc: &dyn JsonRpc,
    signature: &str,
    opts: ConfirmOptions,
) -> Result<Confirmation, SubmitError> {
    for poll in 0..opts.max_polls {
        if poll > 0 {
            tokio::time::sleep(opts.poll_interval).await;
        }

        let result = rpc
            .call(
                "getSignatureStatuses",
                json!([[signature], { "searchTransactionHistory": true }]),
            )
            .await?;

        let status = result
            .get("value")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .ok_or_else(|| SubmitError::Malformed {
                method: "getSignatureStatuses".to_string(),
                detail: "response had no `value` array".to_string(),
            })?;

        // `null` → the cluster hasn't seen it yet; keep polling.
        if status.is_null() {
            continue;
        }

        // A landed tx that FAILED carries a non-null `err`.
        if let Some(err) = status.get("err") {
            if !err.is_null() {
                return Err(SubmitError::TxFailed {
                    signature: signature.to_string(),
                    error: err.to_string(),
                });
            }
        }

        let reached = status
            .get("confirmationStatus")
            .and_then(Value::as_str)
            .unwrap_or("");
        let accepted = if opts.require_finalized {
            reached == "finalized"
        } else {
            reached == "confirmed" || reached == "finalized"
        };
        if accepted {
            return Ok(Confirmation {
                signature: signature.to_string(),
                confirmation_status: reached.to_string(),
            });
        }
        // Otherwise it's `processed` (below target) — keep polling.
    }

    Err(SubmitError::ConfirmTimeout {
        signature: signature.to_string(),
        polls: opts.max_polls,
        seconds: opts.poll_interval.as_secs() * opts.max_polls as u64,
    })
}

/// The full keeper step: fetch a blockhash, build+sign the `submit_ai_claim`
/// transaction with `authority`, send it, and confirm it — returning the
/// [`Confirmation`] (signature + status) or a clear [`SubmitError`].
pub async fn submit_and_confirm(
    rpc: &dyn JsonRpc,
    oracle: &Pubkey,
    proposer: &Pubkey,
    authority: &Keypair,
    payload: &[u8; SUBMIT_AI_CLAIM_PAYLOAD_LEN],
    opts: ConfirmOptions,
) -> Result<Confirmation, SubmitError> {
    let blockhash = get_latest_blockhash(rpc).await?;
    let tx = build_signed_transaction(oracle, proposer, authority, payload, blockhash);
    let tx_base64 = encode_transaction(&tx);
    let signature = send_transaction(rpc, &tx_base64).await?;
    confirm(rpc, &signature, opts).await
}
