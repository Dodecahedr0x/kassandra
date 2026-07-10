//! On-chain Oracle/Fact fetch over Solana JSON-RPC + the off-chain
//! prompt-text-by-hash source (Task I3).
//!
//! The runner can build its config from an oracle pubkey instead of an explicit
//! full config: it reads the `Oracle` account (and its agreed `Fact` accounts)
//! straight off chain and decodes them through the SHARED
//! `kassandra_oracles_sdk::accounts` `Pod` structs — zero new decode code. The
//! interpretation TEXT is NOT on chain; it lives in the `oracle_meta` uri JSON,
//! bound by `uri_hash`. [`fetch_oracle_meta`] reads the (program-owned) meta
//! account; the caller (`build_config_from_chain`) fetches the uri and verifies
//! `sha256(json) == uri_hash` before using it — the same fetch-by-uri, check-the-
//! hash contract as the fact content.
//!
//! # Transport (no solana-client)
//!
//! Everything is plain JSON-RPC over the same `reqwest` stack the fact fetcher
//! uses — no `solana-client`/`solana-sdk` dependency. Requests go through the
//! [`JsonRpc`] trait so the whole decode path is exercised OFFLINE in tests via
//! [`MockRpc`] (mirroring [`crate::fetch::MockFactFetcher`]).
//!
//! # Account validation before decode
//!
//! Every fetched account is validated before it is `bytemuck`-decoded:
//! - the account MUST be owned by the Kassandra program (`kassandra_oracles_sdk::PROGRAM_ID`);
//! - its first byte (the [`AccountType`] tag) MUST match the expected type;
//! - its length MUST be at least the struct's `LEN`.
//!
//! This rejects type-confusion (a `Fact` handed where an `Oracle` is expected)
//! and foreign accounts before any bytes are interpreted.
//!
//! # Enumerating an oracle's agreed facts
//!
//! A `Fact` PDA is `[b"fact", oracle, content_hash]`, so the fact set CANNOT be
//! enumerated from the oracle alone (the `content_hash`es aren't known up
//! front). Instead [`fetch_agreed_facts`] uses `getProgramAccounts` with a
//! `dataSize == Fact::LEN` filter plus a `memcmp` on the `Fact.oracle` field
//! (offset [`FACT_ORACLE_OFFSET`]) to pull exactly this oracle's `Fact`
//! accounts, decodes each, and keeps the ones whose `agreed` flag is set.

mod client;
mod fetch;
mod mock;

#[cfg(test)]
mod tests;

pub use client::*;
pub use fetch::*;
pub use mock::*;
