use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::constants::*;
use crate::errors::EscrowError;
use crate::state::*;

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    pub payment_mint: Account<'info, Mint>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<InitializeConfig>,
    fee_bps: u16,
    timeout_seconds: i64,
) -> Result<()> {
    require!(fee_bps <= MAX_FEE_BPS, EscrowError::FeeTooHigh);
    require!(
        (MIN_TIMEOUT..=MAX_TIMEOUT).contains(&timeout_seconds),
        EscrowError::InvalidTimeout
    );

    let config = &mut ctx.accounts.config;
    config.admin = ctx.accounts.admin.key();
    config.treasury = ctx.accounts.admin.key();
    config.payment_mint = ctx.accounts.payment_mint.key();
    config.fee_bps = fee_bps;
    config.timeout_seconds = timeout_seconds;
    config.total_escrows = 0;
    config.total_volume = 0;
    config.total_fees_collected = 0;
    config.paused = false;
    config.bump = ctx.bumps.config;

    msg!(
        "GhostMarket initialized | mint: {} | fee: {} bps | timeout: {}s",
        config.payment_mint,
        fee_bps,
        timeout_seconds
    );

    Ok(())
}
