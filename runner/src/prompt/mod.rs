//! Deterministic prompt assembly + categorical answer parsing (Task R2).
//!
//! This module turns an oracle's resolution rules + the agreed fact set + the
//! categorical options into the **canonical** `system` / `user` strings of a
//! [`CompletionRequest`], defines the **structured-output JSON schema** the real
//! provider (Task R4) forces the model to answer in, and **parses** the model's
//! structured response back into a validated `option_index`.
//!
//! # Why byte-determinism matters here
//!
//! The assembled `system` / `user` strings are committed on-chain via `io_hash`
//! (see [`crate::hashing`] / `runner/HASHING.md`): `io_hash` hashes the EXACT
//! `system` / `user` bytes this module produces. A challenger who assembles from
//! the same inputs MUST produce byte-identical strings, or their `io_hash` will
//! not match and the protocol breaks. Therefore the assembly is fully
//! deterministic:
//!
//! - **Fact ordering is canonical**: facts are sorted by their 32-byte
//!   `content_hash` ascending (lexicographic), so the rendered order is
//!   independent of the caller's input order.
//! - **Fixed separators**: blocks are joined by exactly `"\n\n"`, option lines
//!   by exactly `"\n"`; there is no trailing whitespace and no trailing newline.
//! - **No nondeterminism sources**: no map iteration, no floats, no locale, no
//!   timestamps. Integers are rendered in base-10 with `{}` (locale-independent
//!   for integers). Verified fact content is rendered **verbatim** (it is what
//!   `content_hash` commits to — trimming would diverge from the on-chain hash).
//!
//! # Versioning
//!
//! This file defines **prompt-assembly version 1**, pinned by
//! [`crate::hashing::PROMPT_ASSEMBLY_VERSION`] (re-exported here as
//! [`PROMPT_ASSEMBLY_VERSION`]), which is folded into `params_hash`. **Any change
//! to the assembled bytes — the preamble text, section headers, separators, fact
//! rendering, option enumeration, or the answer instruction — MUST bump that
//! constant**, so claims produced by different assembly versions never collide.
//! The [`assembly_regression_anchor`](tests) test pins the exact assembled output
//! of a fixed input so an accidental format change fails the build instead of
//! silently shipping with the wrong version.
//!
//! # The structured-output schema
//!
//! [`output_schema`] returns the JSON Schema that forces the model to answer
//! `{ "option_index": <integer in [0, count)> }`. Its stable identity is pinned
//! by [`crate::hashing::OUTPUT_SCHEMA_ID`] / [`crate::hashing::OUTPUT_SCHEMA_VERSION`]
//! (also folded into `params_hash`); the only input-dependent part is the
//! `maximum` bound, derived from `options_count` (itself committed on-chain).
//!
//! # Parsing policy
//!
//! [`parse_option_index`] is **lenient about extra fields** (it reads only
//! `option_index` and ignores any others — though the schema's
//! `additionalProperties: false` prevents extras from a compliant provider) but
//! **strict about the value**: it rejects missing / non-object / non-integer /
//! negative / out-of-range values with a clear [`ParseError`].

mod build;
mod parse;

/// Re-export of the assembly version constant (lives in [`crate::hashing`] so it
/// can feed `params_hash`). This module's format IS that version's contract;
/// changing the format requires bumping the constant there.
pub use crate::hashing::PROMPT_ASSEMBLY_VERSION;

pub use build::{assemble, build_request, AssembledPrompt, Fact, SYSTEM_PREAMBLE};
pub use parse::{output_schema, parse_option_index, ParseError};
