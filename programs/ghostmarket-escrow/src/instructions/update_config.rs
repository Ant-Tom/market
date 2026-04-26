use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::EscrowError;
use crate::state::Config;

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
        has_one = admin @ EscrowError::NotAdmin
    )]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handler(
    ctx: Context<UpdateConfig>,
    fee_bps: Option<u16>,
    timeout_seconds: Option<i64>,
    treasury: Option<Pubkey>,
    paused: Option<bool>,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    if let Some(bps) = fee_bps {
        require!(bps <= MAX_FEE_BPS, EscrowError::FeeTooHigh);
        config.fee_bps = bps;
    }

    if let Some(t) = timeout_seconds {
        require!(
            (MIN_TIMEOUT..=MAX_TIMEOUT).contains(&t),
            EscrowError::InvalidTimeout
        );
        config.timeout_seconds = t;
    }

    if let Some(t) = treasury {
        config.treasury = t;
    }

    if let Some(p) = paused {
        config.paused = p;
    }

    Ok(())
}
