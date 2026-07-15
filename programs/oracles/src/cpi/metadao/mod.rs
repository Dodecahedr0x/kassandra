//! MetaDAO `conditional_vault` + `amm` CPI wire format.
//!
//! # Resolved program IDs (authoritatively sourced — see `scripts/fetch-metadao.sh`)
//!
//! | program            | id                                            | version              |
//! |--------------------|-----------------------------------------------|----------------------|
//! | `conditional_vault`| `VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg` | v0.4.0               |
//! | `amm`              | `AMMyu265tkBpRW21iGQxKGLaves3gKm2JcMUqfXNSpqD` | v0.4.2 (delayed-twap)|
//!
//! Source of truth: `github.com/metaDAOproject/programs` — the `declare_id!`s in
//! `programs/conditional_vault/src/lib.rs` and `programs/amm/src/lib.rs`,
//! cross-checked against `Anchor.toml` and the live mainnet-beta deployments. The
//! DEPLOYED `amm` binary (`AMMyu…`, dumped at mainnet slot 326427490) is the
//! **delayed-twap** v0.4.1/v0.4.2 build (tags `delayed-twap-v0.4.1`,
//! `proposal-duration-v0.4.2`), NOT the base `v0.4` tag — it added
//! `TwapOracle.start_delay_slots` + `CreateAmmArgs.twap_start_delay_slots` (see
//! the `Amm` layout block in [`layout`]). MetaDAO governance v0.5+ moved AMM
//! liquidity to Meteora DAMM v2 (`programs/damm_v2_cpi`), so `AMMyu…` is the last
//! first-party MetaDAO AMM and the one whose built-in TWAP oracle matches our
//! design.
//!
//! # Anchor discriminators
//!
//! Each instruction is selected by `sha256("global:<snake_case_name>")[..8]`.
//! Anchor args follow the discriminator, Borsh-encoded. For the structs we use
//! here the Borsh encoding is just the fields concatenated in declaration order
//! (fixed-size arrays / scalars, no length prefixes), so we hand-roll it to
//! avoid pulling `borsh` into the on-chain program.
//!
//! # `#[event_cpi]`
//!
//! Every `conditional_vault` (and `amm`) instruction is annotated
//! `#[event_cpi]`, which appends **two trailing accounts** to the declared
//! account list: the `event_authority` PDA (seeds `[b"__event_authority"]`,
//! derived under the *target* program) and the target program itself. Anchor's
//! remaining-account loops (e.g. the conditional-token mints) run *after* those
//! two accounts. Account orderings below already include them.

#![allow(dead_code)]

mod encode;
mod layout;
mod pda;
mod wire;

pub use encode::*;
pub use layout::*;
pub use pda::*;
pub use wire::*;
