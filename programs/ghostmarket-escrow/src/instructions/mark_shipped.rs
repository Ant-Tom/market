use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::EscrowError;
use crate::state::*;

#[derive(Accounts)]
pub struct MarkShipped<'info> {
    #[account(
        mut,
        seeds = [
            ESCROW_SEED,
            escrow.buyer.as_ref(),
            escrow.seller.as_ref(),
            &escrow.listing_id,
        ],
        bump = escrow.bump,
        has_one = seller @ EscrowError::NotSeller,
    )]
    pub escrow: Account<'info, Escrow>,

    pub seller: Signer<'info>,
}

pub fn handler(
    ctx: Context<MarkShipped>,
    tracking_hash: [u8; 32],
) -> Result<()> {
    let escrow = &mut ctx.accounts.escrow;

    require!(
        escrow.status == EscrowStatus::Pending,
        EscrowError::InvalidEscrowState
    );

    let now = Clock::get()?.unix_timestamp;
    escrow.tracking_hash = tracking_hash;
    escrow.status = EscrowStatus::Shipped;
    escrow.shipped_at = now;

    emit!(EscrowShipped {
        escrow: escrow.key(),
        tracking_hash,
        shipped_at: now,
    });

    msg!("Escrow {} marked shipped at {}", escrow.key(), now);
    Ok(())
}
