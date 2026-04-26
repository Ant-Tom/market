use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    pub treasury: Pubkey,
    pub payment_mint: Pubkey,
    pub fee_bps: u16,
    pub timeout_seconds: i64,
    pub total_escrows: u64,
    pub total_volume: u64,
    pub total_fees_collected: u64,
    pub paused: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Escrow {
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub payment_mint: Pubkey,
    pub listing_id: [u8; 32],
    pub amount: u64,
    pub fee_amount: u64,
    pub tracking_hash: [u8; 32],
    pub status: EscrowStatus,
    pub created_at: i64,
    pub shipped_at: i64,
    pub finalized_at: i64,
    pub timeout_at: i64,
    pub bump: u8,
    pub vault_bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum EscrowStatus {
    Pending,
    Shipped,
    Completed,
    Refunded,
    Cancelled,
}

impl EscrowStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Completed | Self::Refunded | Self::Cancelled)
    }
}

#[event]
pub struct EscrowCreated {
    pub escrow: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub listing_id: [u8; 32],
    pub amount: u64,
    pub fee_amount: u64,
    pub timeout_at: i64,
}

#[event]
pub struct EscrowShipped {
    pub escrow: Pubkey,
    pub tracking_hash: [u8; 32],
    pub shipped_at: i64,
}

#[event]
pub struct EscrowCompleted {
    pub escrow: Pubkey,
    pub buyer: Pubkey,
    pub seller: Pubkey,
    pub amount_to_seller: u64,
    pub fee_amount: u64,
}

#[event]
pub struct EscrowRefunded {
    pub escrow: Pubkey,
    pub buyer: Pubkey,
    pub amount: u64,
    pub reason: RefundReason,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub enum RefundReason {
    Timeout,
    SellerCancel,
    BuyerCancelBeforeShip,
}
