use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{transfer_checked, close_account, CloseAccount, Mint, Token, TokenAccount, TransferChecked},
};

use crate::constants::*;
use crate::errors::EscrowError;
use crate::state::*;

#[derive(Accounts)]
pub struct ConfirmReceived<'info> {
    #[account(
        mut,
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
        has_one = buyer @ EscrowError::NotBuyer,
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
        payer = buyer,
        associated_token::mint = payment_mint,
        associated_token::authority = seller_pubkey,
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    /// CHECK: верифицируется через escrow.seller
    #[account(address = escrow.seller)]
    pub seller_pubkey: UncheckedAccount<'info>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = payment_mint,
        associated_token::authority = treasury_pubkey,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    /// CHECK: верифицируется через config.treasury
    #[account(address = config.treasury)]
    pub treasury_pubkey: UncheckedAccount<'info>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ConfirmReceived>) -> Result<()> {
    let escrow = &mut ctx.accounts.escrow;

    require!(
        matches!(escrow.status, EscrowStatus::Pending | EscrowStatus::Shipped),
        EscrowError::InvalidEscrowState
    );

    let amount_to_seller = escrow.amount;
    let fee_amount = escrow.fee_amount;
    let decimals = ctx.accounts.payment_mint.decimals;
    let escrow_key = escrow.key();

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
                to: ctx.accounts.seller_token_account.to_account_info(),
                authority: escrow.to_account_info(),
            },
            signer_seeds,
        ),
        amount_to_seller,
        decimals,
    )?;

    if fee_amount > 0 {
        transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.payment_mint.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: escrow.to_account_info(),
                },
                signer_seeds,
            ),
            fee_amount,
            decimals,
        )?;
    }

    close_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        CloseAccount {
            account: ctx.accounts.vault.to_account_info(),
            destination: ctx.accounts.buyer.to_account_info(),
            authority: escrow.to_account_info(),
        },
        signer_seeds,
    ))?;

    let config = &mut ctx.accounts.config;
    config.total_volume = config.total_volume.saturating_add(amount_to_seller);
    config.total_fees_collected = config.total_fees_collected.saturating_add(fee_amount);

    emit!(EscrowCompleted {
        escrow: escrow_key,
        buyer: escrow.buyer,
        seller: escrow.seller,
        amount_to_seller,
        fee_amount,
    });

    msg!(
        "Escrow {} completed | seller: {} | fee: {}",
        escrow_key,
        amount_to_seller,
        fee_amount
    );

    Ok(())
}
