use anchor_lang::prelude::*;

#[error_code]
pub enum EscrowError {
    #[msg("Fee BPS exceeds maximum allowed (1500 = 15%)")]
    FeeTooHigh,

    #[msg("Timeout is outside allowed range (1d-60d)")]
    InvalidTimeout,

    #[msg("Amount must be greater than zero")]
    ZeroAmount,

    #[msg("Buyer and seller cannot be the same address")]
    SelfPurchase,

    #[msg("Escrow is in invalid state for this operation")]
    InvalidEscrowState,

    #[msg("Only the buyer can perform this action")]
    NotBuyer,

    #[msg("Only the seller can perform this action")]
    NotSeller,

    #[msg("Only admin can perform this action")]
    NotAdmin,

    #[msg("Escrow timeout has not yet expired")]
    TimeoutNotReached,

    #[msg("Escrow has already been cancelled or completed")]
    EscrowFinalized,

    #[msg("Cannot cancel: seller has already shipped")]
    AlreadyShipped,

    #[msg("Math overflow")]
    MathOverflow,

    #[msg("Token mint does not match config")]
    InvalidMint,

    #[msg("Marketplace is paused")]
    Paused,

    #[msg("Token account owner mismatch")]
    InvalidTokenOwner,
}
