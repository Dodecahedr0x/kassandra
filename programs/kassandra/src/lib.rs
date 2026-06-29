#![allow(unexpected_cfgs)]
use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

// `entrypoint!` also installs the default allocator and panic handler.
// Gated so the crate can be reused as a plain library (CPI helpers,
// discriminators, etc.) without emitting a second program entrypoint.
#[cfg(not(feature = "no-entrypoint"))]
use pinocchio::entrypoint;

#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);

pub mod state;

pub const ID: Pubkey = pinocchio_pubkey::pubkey!("KassVxvXUEPr5apSr2MqiGva4VFtJXyYLLDFS3f83nY");

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }
    Ok(())
}
