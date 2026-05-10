use anchor_lang::prelude::*;

declare_id!("FVi3CE8X75fAZ5x1MPnwJ2UikDUe6go4unT7iQiCxzok");

#[program]
pub mod shellshock {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
