//! The `run` / `verify` CLI (Task R4).
//!
//! Wires the R0–R3 pieces into two commands:
//!
//! - **`run`**: load an oracle config → fetch + verify the agreed facts
//!   ([`crate::fetch`]) → assemble the prompt ([`crate::prompt`]) → call the
//!   provider ([`crate::provider`]) → compute the claim metadata
//!   ([`crate::hashing`]) → emit the `option`, the three hashes (hex), and the
//!   97-byte `submit_ai_claim` payload (hex) as JSON on stdout.
//! - **`verify`**: re-run for the same config and compare the produced option to
//!   a submitted claim's option (and, optionally, the submitted hashes) →
//!   advise "matches (no challenge)" vs "differs (consider challenging)".
//!
//! The default provider is Anthropic (Claude); `--mock` (or the
//! `KASSANDRA_RUNNER_MOCK` env var) selects the deterministic
//! [`crate::provider::MockProvider`] so the CLI runs offline with no API key.
//!
//! # Config shape
//!
//! The input config is JSON, read from `--config <path>` or stdin:
//!
//! ```json
//! {
//!   "interpretation": "Resolve YES if BTC closed above $100k on the date; otherwise NO.",
//!   "options_count": 2,
//!   "option_labels": [ { "index": 0, "label": "Yes" }, { "index": 1, "label": "No" } ],
//!   "facts": [
//!     { "content_hash": "<64-hex sha256 of the content>", "uri": "https://..." }
//!   ],
//!   "oracle": "<oracle pubkey, optional — only used to echo the AiClaim PDA seeds>",
//!   "proposer": "<proposer pubkey, optional>"
//! }
//! ```
//!
//! `option_labels`, `oracle`, and `proposer` are optional. `content_hash` is the
//! off-chain `sha256(content)` convention (see [`crate::fetch`]); the runner
//! recomputes it over the fetched bytes and rejects any mismatch.
//!
//! # Determinism caveat
//!
//! Re-running `verify` reproduces the same categorical option only as far as the
//! model is deterministic — no frontier API is bit-reproducible. The point of
//! `verify` is the categorical comparison plus confirming the inputs + metadata
//! reproduce: identical `model_id` / `params_hash` (deterministic), and an
//! `io_hash` that commits to the exact (input, raw response) seen.

mod args;
mod config;
mod output;
mod run;

#[cfg(test)]
mod tests;

pub use args::*;
pub use config::*;
pub use output::*;
pub use run::*;
