//! CPI caller for the missing-input guard tests: deliberately catch a CPI error.
//! Adapted to the existing fixture SDK from Apache-2.0 Agave commit
//! 88fb4364021e180e98f8cfddcafe5a33b9c7e2a9, programs/sbf/rust/invoke_and_ok.
use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    pubkey::Pubkey,
};

entrypoint!(process_instruction);

fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = Instruction {
        accounts: accounts[1..]
            .iter()
            .map(|account| AccountMeta {
                pubkey: *account.key,
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data: instruction_data.to_owned(),
        program_id: *accounts[0].key,
    };
    // This catch is the behavior under test: the host must still veto missing input.
    let _caught_cpi_error = invoke(&instruction, accounts);
    Ok(())
}
