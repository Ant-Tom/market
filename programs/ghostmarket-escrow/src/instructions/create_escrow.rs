use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer_checked, Mint, Token, TokenAccount, TransferChecked},
};

use crate::constants::*;
use crate::errors::EscrowError;
use crate::state::*;

#[derive(Accounts)]
#[instruction(listing_id: [u8; 32], amount: u64)]
pub struct CreateEscrow<'info> {
    #[account(
        mut,
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,

    #[account(
        init,
        payer = buyer,
        space = 8 + Escrow::INIT_SPACE,
        seeds = [
            ESCROW_SEED,
            buyer.key().as_ref(),
            seller.key().as_ref(),
            &listing_id,
        ],
        bump
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(
        init,
        payer = buyer,
        seeds = [VAULT_SEED, escrow.key().as_ref()],
        bump,
        token::mint = payment_mint,
        token::authority = escrow,
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        address = config.payment_mint @ EscrowError::InvalidMint
    )]
    pub payment_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = payment_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    /// CHECK: только pubkey, не подписант
    pub seller: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(
    ctx: Context<CreateEscrow>,
    listing_id: [u8; 32],
    amount: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    require!(!config.paused, EscrowError::Paused);
    require!(amount > 0, EscrowError::ZeroAmount);
    require!(
        ctx.accounts.buyer.key() != ctx.accounts.seller.key(),
        EscrowError::SelfPurchase
    );

    let fee_amount = (amount as u128)
        .checked_mul(config.fee_bps as u128)
        .ok_or(EscrowError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(EscrowError::MathOverflow)? as u64;

    let total = amount.checked_add(fee_amount).ok_or(EscrowError::MathOverflow)?;

    let now = Clock::get()?.unix_timestamp;
    let timeout_at = now
        .checked_add(config.timeout_seconds)
        .ok_or(EscrowError::MathOverflow)?;

    let escrow = &mut ctx.accounts.escrow;
    escrow.buyer = ctx.accounts.buyer.key();
    escrow.seller = ctx.accounts.seller.key();
    escrow.payment_mint = ctx.accounts.payment_mint.key();
    escrow.listing_id = listing_id;
    escrow.amount = amount;
    escrow.fee_amount = fee_amount;
    escrow.tracking_hash = [0u8; 32];
    escrow.status = EscrowStatus::Pending;
    escrow.created_at = now;
    escrow.shipped_at = 0;
    escrow.finalized_at = 0;
    escrow.timeout_at = timeout_at;
    escrow.bump = ctx.bumps.escrow;
    escrow.vault_bump = ctx.bumps.vault;

    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        TransferChecked {
            from: ctx.accounts.buyer_token_account.to_account_info(),
            mint: ctx.accounts.payment_mint.to_account_info(),
            to: ctx.accounts.vault.to_account_info(),
            authority: ctx.accounts.buyer.to_account_info(),
        },
    );
    transfer_checked(cpi_ctx, total, ctx.accounts.payment_mint.decimals)?;

    config.total_escrows = config.total_escrows.saturating_add(1);

    emit!(EscrowCreated {
        escrow: escrow.key(),
        buyer: escrow.buyer,
        seller: escrow.seller,
        listing_id,
        amount,
        fee_amount,
        timeout_at,
    });

    msg!(
        "Escrow created | id: {} | amount: {} | fee: {} | timeout_at: {}",
        escrow.key(),
        amount,
        fee_amount,
        timeout_at
    );

    Ok(())
}
