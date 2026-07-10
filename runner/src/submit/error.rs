//! The [`SubmitError`] type covering every build/send/confirm failure.

use crate::rpc::RpcError;

/// Anything that can go wrong building/sending/confirming the claim tx.
#[derive(Debug, thiserror::Error)]
pub enum SubmitError {
    /// The `--keypair` file could not be read.
    #[error("failed to read keypair file `{path}`: {message}")]
    KeypairRead {
        /// The keypair path.
        path: String,
        /// The rendered IO error.
        message: String,
    },
    /// The `--keypair` file was not a valid Solana CLI keypair (a 64-byte JSON
    /// array of a ed25519 keypair).
    #[error("keypair file `{path}` is malformed: {message}")]
    KeypairMalformed {
        /// The keypair path.
        path: String,
        /// What was wrong.
        message: String,
    },
    /// A transport / JSON-RPC error (including a `sendTransaction` PREFLIGHT
    /// failure, which the RPC returns as a JSON-RPC error carrying the program
    /// error — e.g. an already-submitted claim or a wrong-phase reject).
    #[error(transparent)]
    Rpc(#[from] RpcError),
    /// An RPC response did not have the expected shape.
    #[error("malformed `{method}` response: {detail}")]
    Malformed {
        /// The RPC method.
        method: String,
        /// What was wrong.
        detail: String,
    },
    /// The transaction was landed but FAILED on chain (its status carried an
    /// `err`) — e.g. a program error that slipped past preflight.
    #[error("transaction `{signature}` failed on chain: {error}")]
    TxFailed {
        /// The transaction signature (base58).
        signature: String,
        /// The on-chain error (the JSON `err` object, rendered).
        error: String,
    },
    /// The transaction was sent but did not reach the target commitment within
    /// the poll budget.
    #[error("transaction `{signature}` not confirmed after {polls} polls (~{seconds}s)")]
    ConfirmTimeout {
        /// The transaction signature (base58).
        signature: String,
        /// How many status polls were made.
        polls: u32,
        /// The elapsed wall-clock budget, seconds.
        seconds: u64,
    },
}
