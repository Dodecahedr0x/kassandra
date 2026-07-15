//! MetaDAO `conditional_vault` (v0.4.0) + `amm` (v0.4.2) CPI wire format —
//! client-side composition. This is the SINGLE SOURCE OF TRUTH for the
//! discriminators, PDA seeds, and account orders the keeper/test harness uses to
//! compose the MetaDAO market BEFORE calling `kassandra-market::activate`. The
//! program crate re-declares only the subset it invokes (`split_tokens`,
//! `add_liquidity`) and `tests/parity.rs` asserts they agree byte-for-byte.
//!
//! Ported + re-verified against `../kassandra/programs/oracles/src/cpi/metadao.rs`
//! and the real account orders realized in `../kassandra/programs/oracles/tests/
//! challenge_e2e.rs` (`build_pool`, `setup_market`).

mod builders;
mod data;
mod disc;
mod ids;
mod pda;

pub use builders::*;
pub use data::*;
pub use disc::*;
pub use ids::*;
pub use pda::*;
