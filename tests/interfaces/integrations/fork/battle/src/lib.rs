//! Battle-test program for historical forks: a small interpreter whose instruction data is a list of operations on the
//! accounts the transaction passes it. One program can then compose any interaction with mainnet programs and
//! accounts: cross-program calls (also signed by its own address), account creation, writes, lamport moves, re-entry
//! and deliberate failure. Operations, by first byte:
//!   1 invoke:        program, n, n x (account, flags: 1 signer, 2 writable), u16 length, data
//!   2 invoke signed: seed length, seed, then as 1; signed by this program's address of [seed]
//!   3 create:        payer, new account, u16 space, seed length, seed (rent-exempt, owned by this program)
//!   4 write:         account, u16 offset, u16 length, bytes
//!   5 move lamports: from (owned by this program), to, u64 amount
//!   6 log the return data of the last call
//!   7 fail:          u32 custom error code
//!   8 log account:   account (address, lamports, owner, length, and its data as program data)
//!   9 log the clock
mod operations;
mod reader;

use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, pubkey::Pubkey,
};

entrypoint!(process);

fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let mut r = reader::Reader { data, at: 0 };
    while r.at < data.len() {
        operations::run(program_id, accounts, &mut r)?;
    }
    Ok(())
}
