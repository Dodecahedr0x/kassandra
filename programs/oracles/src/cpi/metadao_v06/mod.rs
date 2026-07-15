//! MetaDAO **futarchy v0.6** + **Meteora DAMM v2** CPI wire format + account
//! layouts (Task F0 recon).
//!
//! This is the v0.6 governance counterpart of [`super::metadao`] (which pins the
//! dispute core's v0.4 standalone `amm` + `conditional_vault`). v0.6 is a
//! SEPARATE, NEWER stack and this module is purely ADDITIVE — it does not touch
//! the v0.4 wiring.
//!
//! # Resolved program IDs (authoritatively sourced — see `scripts/fetch-metadao-v06.sh`)
//!
//! | program            | id                                            | version / source                |
//! |--------------------|-----------------------------------------------|---------------------------------|
//! | `futarchy`         | `FUTARELBfJfQ8RDGhg1wdhddq1odMAJUePHFuBYfUxKq` | v0.6.0 (replaces `autocrat`)     |
//! | `conditional_vault`| `VLTX1ishMBbcX3rdBWGssxawAo1Q2X2qxYFYqiGodVg` | v0.6 line — UNCHANGED from v0.4  |
//! | Meteora DAMM v2    | `cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG` | cp-amm (MeteoraAg/damm-v2 @ main)|
//! | Squads v4          | `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf` | DAO execution authority host    |
//!
//! Source of truth:
//! * `github.com/metaDAOproject/programs` @ tag **v0.6.0**: `Anchor.toml`
//!   `[programs.localnet]` + the `declare_id!`s in `programs/futarchy/src/lib.rs`
//!   and `programs/conditional_vault/src/lib.rs`, cross-checked against the live
//!   mainnet-beta deployments (slots/sizes in the fetch script header).
//! * `github.com/MeteoraAg/damm-v2` `programs/cp-amm/src/lib.rs` @ main for the
//!   Meteora DAMM v2 (cp-amm) `declare_id!`, cross-confirmed as the mainnet
//!   deployment in MeteoraAg/damm-v2-sdk. MetaDAO's `programs/damm_v2_cpi` shim
//!   (v0.6 tree) `declare_id!`s the same `cpamd…` address.
//! * `github.com/Squads-Protocol/v4` @ rev `6d5235da621a2e9b7379ea358e48760e981053be`
//!   (the exact rev `futarchy/Cargo.toml` depends on) for the multisig/vault PDA
//!   seeds (`state/seeds.rs`) and program id (`declare_id!`).
//!
//! # KEY RECON FINDINGS (these drive F1/F5/F6)
//!
//! 1. **DAO execution authority is a Squads v4 multisig vault, not a futarchy
//!    PDA.** `initialize_dao` CPIs into Squads to create a multisig whose
//!    `create_key` is the `Dao` PDA; a passed proposal carries a `squads_proposal`
//!    and executes through the Squads **vault** PDA. So Kassandra's
//!    `Protocol.dao_authority` (the signer of `set_config`/`resolve_deadend`) is
//!    the [`squads_vault_pda`], derived under [`SQUADS_V4_ID`]. See the PDA
//!    builders below for the exact seeds.
//!
//! 2. **Meteora cp-amm has NO TWAP oracle.** Its `Pool` (zero-copy) stores only
//!    an INSTANTANEOUS `sqrt_price: u128` (Q64.64) plus cumulative *fee*
//!    accumulators — there is no cumulative price observation. The
//!    manipulation-resistant KASS/USDC TWAP the design's `kass_price` (F5) needs
//!    is the futarchy program's **embedded** `FutarchyAmm` spot-pool
//!    `TwapOracle` (`Dao.amm` → see [`futarchy_spot_twap`]), NOT Meteora. The
//!    Meteora `Pool` layout is documented below for completeness (an instantaneous
//!    spot price is still a usable, if manipulable, fallback), but F5 should read
//!    the futarchy TWAP.
//!
//! # Anchor discriminators
//!
//! Each instruction is selected by `sha256("global:<snake_case_name>")[..8]`;
//! each account's first 8 bytes are `sha256("account:<TypeName>")[..8]`. Args
//! follow the discriminator, Borsh-encoded.
//!
//! # `#[event_cpi]`
//!
//! The futarchy instructions are `#[event_cpi]`, appending two trailing accounts
//! (the futarchy `event_authority` PDA `[b"__event_authority"]` + the futarchy
//! program id). See [`super::metadao`] for the same mechanism on the vault.

#![allow(dead_code)]

mod encode;
mod layout;
mod pda;
mod wire;

pub use encode::*;
pub use layout::*;
pub use pda::*;
pub use wire::*;
