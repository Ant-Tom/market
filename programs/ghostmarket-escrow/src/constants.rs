use anchor_lang::prelude::*;

#[constant]
pub const CONFIG_SEED: &[u8] = b"config";

#[constant]
pub const ESCROW_SEED: &[u8] = b"escrow";

#[constant]
pub const VAULT_SEED: &[u8] = b"vault";

pub const MAX_FEE_BPS: u16 = 1500;

pub const MIN_TIMEOUT: i64 = 60 * 60 * 24;

pub const MAX_TIMEOUT: i64 = 60 * 60 * 24 * 60;

pub const DEFAULT_FEE_BPS: u16 = 800;

pub const DEFAULT_TIMEOUT: i64 = 60 * 60 * 24 * 14;

pub const BPS_DENOMINATOR: u64 = 10_000;
