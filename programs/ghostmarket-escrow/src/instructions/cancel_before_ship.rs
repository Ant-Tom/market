use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer_checked, close_account, CloseAccount, Mint, Token, TokenAccount, TransferChecked},
};

use crate::constants::*;
use crate::errors::EscrowError;
use crate::state::*;

#[derive(Accounts)]
pub struct CancelBeforeShip<'info> {
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [
            ESCROW_SEED,
            escrow.buyer.as_ref(),
            escrow.seller.as_ref(),
            &escrow.listing_id,
        ],
        bump = escrow.bump,
        close = buyer,
    )]
    pub escrow: Account<'info, Escrow>,

    #[account(
        mut,
        seeds = [VAULT_SEED, escrow.key().as_ref()],
        bump = escrow.vault_bump,
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        address = config.payment_mint @ EscrowError::InvalidMint
    )]
    pub payment_mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = buyer_signer_payer,
        associated_token::mint = payment_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    /// CHECK: верифицируется через escrow.buyer; mut нужен для close = buyer
    #[account(mut, address = escrow.buyer)]
    pub buyer: UncheckedAccount<'info>,

    /// CHECK: либо buyer, либо seller; платит за ATA если её нет.
    /// Если signer == buyer, должен совпадать с buyer аккаунтом.
    #[account(
        mut,
        constraint = (
            buyer_signer_payer.key() == escrow.buyer
        ) @ EscrowError::InvalidEscrowState
    )]
    pub buyer_signer_payer: Signer<'info>,

    #[account(
        mut,
        constraint = (
            signer.key() == escrow.buyer || signer.key() == escrow.seller
        ) @ EscrowError::InvalidEscrowState
    )]
    pub signer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CancelBeforeShip>) -> Result<()> {
    let escrow = &mut ctx.accounts.escrow;

    require!(
        escrow.status == EscrowStatus::Pending,
        EscrowError::AlreadyShipped
    );

    let total_to_refund = escrow
        .amount
        .checked_add(escrow.fee_amount)
        .ok_or(EscrowError::MathOverflow)?;

    let decimals = ctx.accounts.payment_mint.decimals;
    let escrow_key = escrow.key();
    let signer_key = ctx.accounts.signer.key();

    let signer_seeds: &[&[&[u8]]] = &[&[
        ESCROW_SEED,
        escrow.buyer.as_ref(),
        escrow.seller.as_ref(),
        &escrow.listing_id,
        &[escrow.bump],
    ]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.payment_mint.to_account_info(),
                to: ctx.accounts.buyer_token_account.to_account_info(),
                authority: escrow.to_account_info(),
            },
            signer_seeds,
        ),
        total_to_refund,
        decimals,
    )?;

    close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.buyer.to_account_info(),
            authority: escrow.to_account_info(),
        },
        signer_seeds,
    ))?;

    let now = Clock::get()?.unix_timestamp;
    escrow.status = EscrowStatus::Cancelled;
    escrow.finalized_at = now;

    let reason = if signer_key == escrow.buyer {
        RefundReason::BuyerCancelBeforeShip
    } else {
        RefundReason::SellerCancel
    };

    emit!(EscrowRefunded {
        escrow: escrow_key,
        buyer: escrow.buyer,
        amount: total_to_refund,
        reason,
    });

    msg!("Escrow {} cancelled | initiator: {}", escrow_key, signer_key);
    Ok(())
}
