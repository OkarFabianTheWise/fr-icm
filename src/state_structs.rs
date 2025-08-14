// --- On-chain account structs matching Anchor definitions ---
use anchor_lang::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradingPool {
    pub pool_id: String,
    pub pool_bump: u8,
    pub creator: String,
    pub token_bucket: Vec<String>,
    pub target_amount: String,
    pub min_contribution: String,
    pub max_contribution: String,
    pub trading_duration: String,
    pub created_at: String,
    pub fundraising_deadline: String,
    pub trading_start_time: Option<String>,
    pub trading_end_time: Option<String>,
    pub phase: String,
    pub management_fee: u16,
    // New fields for UI
    pub raised_amount: Option<String>,
    pub contribution_percent: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreatorProfile {
    pub creator: String,
    pub pools_created: u32,
    pub successful_pools: u32,
    pub total_volume_managed: String,
    pub reputation_score: u32,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BucketAccount {
    pub creator: String,
    pub name: String,
    pub token_mints: Vec<String>,
    pub contribution_deadline: String, // Use String for BN compatibility
    pub trading_deadline: String,
    pub creator_fee_percent: u16,
    pub status: String,
    pub trading_started_at: String,
    pub closed_at: String,
    pub bump: u8,
    pub creator_profile: Pubkey, // base58 pubkey
    pub performance_fee: u16,
    pub raised_amount: u64,
    pub contributor_count: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BucketInfo {
    pub public_key: String,
    pub account: BucketAccount,
}

#[derive(Debug, serde::Serialize)]
pub struct UnsignedTransactionResponse {
    pub transaction: String,
    pub message: String,
}


// --- Request structs ---
#[derive(Deserialize)]
pub struct CreateBucketRequest {
    pub name: String,
    pub token_mints: Vec<String>, // base58 pubkeys
    pub contribution_window_days: u32,
    pub trading_window_days: u32,
    pub creator_fee_percent: u16,
    pub target_amount: u64,
    pub min_contribution: u64,
    pub max_contribution: u64,
    pub management_fee: u16,
}

#[derive(Deserialize)]
pub struct ContributeToBucketRequest {
    pub bucket_name: String,
    pub amount: u64,
    pub creator_pubkey: String, // base58 pubkey
}

#[derive(Deserialize)]
pub struct StartTradingRequest {
    pub bucket_name: String,
    pub creator_pubkey: String, // base58 pubkey
}

#[derive(Deserialize)]
pub struct SwapTokensRequest {
    pub bucket: String,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: u64,
    pub quoted_out_amount: u64,
    pub slippage_bps: u16,
    pub platform_fee_bps: u16,
    pub route_plan: Vec<u8>,
}

#[derive(Deserialize)]
pub struct ClaimRewardsRequest {
    pub bucket_name: String,
    pub token_mint: String,
}

#[derive(Deserialize)]
pub struct CloseBucketRequest {
    pub bucket_name: String,
    pub creator_pubkey: String, // base58 pubkey
}

#[derive(Serialize)]
pub struct TxResponse {
    pub success: bool,
    pub tx_signature: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetCreatorProfileQuery {
    pub creator_pubkey: String,
}

// GetBucketQuery
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetBucketQuery {
    pub bucket_name: String,
    pub creator_pubkey: String,
    pub raised_amount: Option<String>,
}