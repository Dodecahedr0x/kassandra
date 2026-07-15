//! Instruction builders — one per [`Ix`] variant, returning a client
//! [`Instruction`]. Account order, signer/writable flags, and payload byte
//! layouts are the program's wire contract (see the processors in
//! `programs/oracles/src/processor/`). Every oracle-PDA-signing instruction
//! needs the oracle `nonce`, which the Oracle struct does not store — callers
//! must carry it alongside the oracle pubkey.
//!
//! [`Ix`]: kassandra_oracles_program::instruction::Ix

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

mod ai_claim;
mod challenge;
mod claims;
mod facts;
mod governance;
mod lifecycle;
mod meta;
mod proposals;
mod setup;

pub use ai_claim::*;
pub use challenge::*;
pub use claims::*;
pub use facts::*;
pub use governance::*;
pub use lifecycle::*;
pub use meta::*;
pub use proposals::*;
pub use setup::*;

#[inline]
fn build(program_id: &Pubkey, accounts: Vec<AccountMeta>, data: Vec<u8>) -> Instruction {
    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}
