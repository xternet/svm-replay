//! The operations: one per first byte of each step of the instruction data.
#![allow(deprecated)] // system_instruction: solana-program 2.3's own create_account builder

use crate::reader::Reader;
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    log::sol_log_data,
    msg,
    program::{get_return_data, invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

fn account<'a, 'b>(
    accounts: &'b [AccountInfo<'a>],
    index: u8,
) -> Result<&'b AccountInfo<'a>, ProgramError> {
    accounts
        .get(usize::from(index))
        .ok_or(ProgramError::NotEnoughAccountKeys)
}

/// The instruction of operations 1 and 2: its program, accounts with their flags, and data.
fn instruction(accounts: &[AccountInfo], r: &mut Reader) -> Result<Instruction, ProgramError> {
    let program = *account(accounts, r.u8()?)?.key;
    let mut metas = Vec::new();
    for _ in 0..r.u8()? {
        let (key, flags) = (*account(accounts, r.u8()?)?.key, r.u8()?);
        metas.push(AccountMeta {
            pubkey: key,
            is_signer: flags & 1 != 0,
            is_writable: flags & 2 != 0,
        });
    }
    let length = usize::from(r.u16()?);
    Ok(Instruction {
        program_id: program,
        accounts: metas,
        data: r.bytes(length)?.to_vec(),
    })
}

/// Runs the next operation.
pub(crate) fn run(program_id: &Pubkey, accounts: &[AccountInfo], r: &mut Reader) -> ProgramResult {
    match r.u8()? {
        1 => invoke(&instruction(accounts, r)?, accounts)?,
        2 => {
            let seed_length = usize::from(r.u8()?);
            let seed = r.bytes(seed_length)?;
            let (_, bump) = Pubkey::find_program_address(&[seed], program_id);
            invoke_signed(&instruction(accounts, r)?, accounts, &[&[seed, &[bump]]])?;
        }
        3 => {
            let (payer, created, space) = (
                account(accounts, r.u8()?)?,
                account(accounts, r.u8()?)?,
                r.u16()?,
            );
            let seed_length = usize::from(r.u8()?);
            let seed = r.bytes(seed_length)?;
            let (address, bump) = Pubkey::find_program_address(&[seed], program_id);
            if address != *created.key {
                return Err(ProgramError::InvalidSeeds);
            }
            let lamports = Rent::get()?.minimum_balance(usize::from(space));
            let create = system_instruction::create_account(
                payer.key,
                created.key,
                lamports,
                u64::from(space),
                program_id,
            );
            invoke_signed(&create, accounts, &[&[seed, &[bump]]])?;
        }
        4 => {
            let (target, offset, length) = (
                account(accounts, r.u8()?)?,
                usize::from(r.u16()?),
                usize::from(r.u16()?),
            );
            let bytes = r.bytes(length)?;
            let mut data = target.try_borrow_mut_data()?;
            data.get_mut(offset..offset + length)
                .ok_or(ProgramError::AccountDataTooSmall)?
                .copy_from_slice(bytes);
        }
        5 => {
            let (from, to, amount) = (
                account(accounts, r.u8()?)?,
                account(accounts, r.u8()?)?,
                r.u64()?,
            );
            let remaining = from
                .lamports()
                .checked_sub(amount)
                .ok_or(ProgramError::InsufficientFunds)?;
            **from.try_borrow_mut_lamports()? = remaining;
            **to.try_borrow_mut_lamports()? = to
                .lamports()
                .checked_add(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?;
        }
        6 => match get_return_data() {
            Some((program, data)) => sol_log_data(&[program.as_ref(), &data]),
            None => msg!("no return data"),
        },
        7 => return Err(ProgramError::Custom(r.u32()?)),
        8 => {
            let target = account(accounts, r.u8()?)?;
            msg!(
                "account {} lamports {} owner {} length {}",
                target.key,
                target.lamports(),
                target.owner,
                target.data_len()
            );
            sol_log_data(&[&target.try_borrow_data()?]);
        }
        9 => {
            let clock = Clock::get()?;
            msg!(
                "clock slot {} epoch {} unix_timestamp {}",
                clock.slot,
                clock.epoch,
                clock.unix_timestamp
            );
        }
        _ => return Err(ProgramError::InvalidInstructionData),
    }
    Ok(())
}
