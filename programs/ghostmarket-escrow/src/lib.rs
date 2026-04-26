use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod state;
pub mod instructions;

use instructions::*;

declare_id!("GHoSTMKtEscRoW1111111111111111111111111111111");

#[program]
pub mod ghostmarket_escrow {
    use super::*;

    pub fn initialize_config(
        ctx: Context<InitializeConfig>,
        fee_bps: u16,
        timeout_seconds: i64,
    ) -> Result<()> {
        instructions::initialize_config::handler(ctx, fee_bps, timeout_seconds)
    }

    pub fn update_config(
        ctx: Context<UpdateConfig>,
        fee_bps: Option<u16>,
        timeout_seconds: Option<i64>,
        treasury: Option<Pubkey>,
        paused: Option<bool>,
    ) -> Result<()> {
        instructions::update_config::handler(ctx, fee_bps, timeout_seconds, treasury, paused)
    }

    pub fn create_escrow(
        ctx: Context<CreateEscrow>,
        listing_id: [u8; 32],
        amount: u64,
    ) -> Result<()> {
        instructions::create_escrow::handler(ctx, listing_id, amount)
    }

    pub fn mark_shipped(
        ctx: Context<MarkShipped>,
        tracking_hash: [u8; 32],
    ) -> Result<()> {
        instructions::mark_shipped::handler(ctx, tracking_hash)
    }

    pub fn confirm_received(ctx: Context<ConfirmReceived>) -> Result<()> {
        instructions::confirm_received::handler(ctx)
    }

    pub fn claim_timeout(ctx: Context<ClaimTimeout>) -> Result<()> {
        instructions::claim_timeout::handler(ctx)
    }

    pub fn cancel_before_ship(ctx: Context<CancelBeforeShip>) -> Result<()> {
        instructions::cancel_before_ship::handler(ctx)
    }
}
