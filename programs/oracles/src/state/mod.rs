//! Fixed-size, zero-copy on-chain account layouts and the dispute phase enum.
//!
//! Every account struct is `#[repr(C)]`, `Pod` + `Zeroable`, and fully packed
//! (no implicit padding): fields are ordered and explicit `_pad` arrays are
//! inserted so each struct's `size_of` is a multiple of its 8-byte alignment.
//! This lets us read/write them straight out of account data with `bytemuck`.

mod accounts;
mod common;
mod oracle;
mod protocol;

pub use accounts::*;
pub use common::*;
pub use oracle::*;
pub use protocol::*;
