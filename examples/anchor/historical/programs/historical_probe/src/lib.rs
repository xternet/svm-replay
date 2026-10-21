use anchor_lang::prelude::*;

declare_id!("2TnwUoEbP8RKeNP3uimWaW78ZEdmJwCRMRsTUmZkMRmh");

/// Reports what a freshly deployed program observes inside a historical fork.
#[program]
pub mod historical_probe {
    use super::*;

    pub fn observe(ctx: Context<Observe>) -> Result<()> {
        let clock = Clock::get()?;
        msg!("unix_timestamp={} slot={}", clock.unix_timestamp, clock.slot);
        msg!("lamports={}", ctx.accounts.target.lamports());
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Observe<'info> {
    /// CHECK: any historical account; only its lamports are read.
    pub target: UncheckedAccount<'info>,
}
