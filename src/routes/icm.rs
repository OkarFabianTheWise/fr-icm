use axum::{Json, extract::{State, Extension}, response::IntoResponse};
use serde::{Deserialize, Serialize};
use crate::server::AppState;
use anchor_client::solana_sdk::signature::Keypair;
use anchor_client::Client;
use anchor_client::Cluster;
use solana_sdk::pubkey::Pubkey;
use std::sync::Arc;
use tracing::error;
use solana_sdk::signature::Signer;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::Json as ResponseJson;
use anyhow::Result;
use anchor_lang::declare_program;
use crate::onchain_instance::instance::{
    UnsignedTransactionResponse, GetBucketQuery, BucketInfo, BucketAccount
};

use std::convert::TryFrom;

declare_program!(icm_program);
const ICM_PROGRAM_ID: Pubkey = icm_program::ID;

// --- Request structs ---
#[derive(Deserialize)]
pub struct CreateBucketRequest {
    pub name: String,
    pub token_mints: Vec<String>, // base58 pubkeys
    pub contribution_window_days: u32,
    pub trading_window_days: u32,
    pub creator_fee_percent: u16,
}

#[derive(Deserialize)]
pub struct ContributeToBucketRequest {
    pub bucket_name: String,
    pub token_mint: String,
    pub amount: u64,
}

#[derive(Deserialize)]
pub struct StartTradingRequest {
    pub bucket_name: String,
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
}

#[derive(Serialize)]
pub struct TxResponse {
    pub success: bool,
    pub tx_signature: Option<String>,
    pub error: Option<String>,
}

/// Standard API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

// --- Helper: Get user keypair from DB by email ---
async fn get_user_keypair_by_email(email: &str, state: &AppState) -> Result<Keypair, String> {
    // Fetch private_key from user_profiles by email (as in auth.rs)
    let pool = state.db.pool();
    let client = pool.get().await.map_err(|_| "DB connection error".to_string())?;
    let row = client
        .query_opt(
            "SELECT private_key FROM user_profiles WHERE email = $1",
            &[&email],
        )
        .await
        .map_err(|_| "Failed to query user private key".to_string())?;
    let privkey_bytes: Vec<i32> = match row {
        Some(row) => row.try_get("private_key").map_err(|_| "private_key column missing".to_string())?,
        None => return Err("User not found".to_string()),
    };
    // Convert Vec<i32> to Vec<u8>
    let privkey_bytes: Vec<u8> = privkey_bytes.into_iter().map(|b| b as u8).collect();
    Keypair::try_from(&privkey_bytes[..]).map_err(|_| "Invalid keypair bytes".to_string())
}

/// Get all pools (buckets) endpoint
#[axum::debug_handler]
pub async fn get_trading_pool(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>
) -> ResponseJson<ApiResponse<Vec<BucketInfo>>> {
    tracing::debug!("[get_trading_pool] Fetching trading pools");
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[get_trading_pool] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<Vec<BucketInfo>>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };

    match state.icm_client.get_trading_pool(&request, keypair).await {
        Ok(all_pools) => ResponseJson(ApiResponse::success(all_pools)),
        Err(e) => {
            tracing::error!("Failed to fetch all pools: {}", e);
            ResponseJson(ApiResponse::<Vec<BucketInfo>>::error(e.to_string()))
        }
    }
}

#[axum::debug_handler]
pub async fn get_all_pools(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>
) -> ResponseJson<ApiResponse<Vec<BucketInfo>>> {
    tracing::debug!("[get_all_pools] Fetching all pools");
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[get_all_pools] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<Vec<BucketInfo>>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    match state.icm_client.get_all_pools(keypair).await {
        Ok(all_pools) => ResponseJson(ApiResponse::success(all_pools)),
        Err(e) => {
            tracing::error!("Failed to fetch all pools: {}", e);
            ResponseJson(ApiResponse::<Vec<BucketInfo>>::error(e.to_string()))
        }
    }
}

/// Create bucket endpoint
#[axum::debug_handler]
pub async fn create_bucket(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<CreateBucketRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[create_bucket] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    // Convert to instance::CreateBucketRequest
    let instance_request = crate::onchain_instance::instance::CreateBucketRequest {
        name: request.name,
        token_mints: request.token_mints,
        contribution_window_days: request.contribution_window_days,
        trading_window_days: request.trading_window_days,
        creator_fee_percent: request.creator_fee_percent,
        creator_pubkey: keypair.pubkey().to_string(),
    };
    match state.icm_client.create_bucket_transaction(instance_request, keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[create_bucket] Create bucket error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            ResponseJson(error_response)
        }
    }
}

/// Contribute to bucket endpoint
#[axum::debug_handler]
pub async fn contribute_to_bucket(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<ContributeToBucketRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[contribute_to_bucket] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    // Convert to instance::ContributeToBucketRequest
    let instance_request = crate::onchain_instance::instance::ContributeToBucketRequest {
        bucket_name: request.bucket_name,
        token_mint: request.token_mint,
        amount: request.amount,
        contributor_pubkey: keypair.pubkey().to_string(),
    };
    match state.icm_client.contribute_to_bucket_transaction(instance_request, keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[contribute_to_bucket] Contribute to bucket error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            ResponseJson(error_response)
        }
    }
}

/// Start trading endpoint
#[axum::debug_handler]
pub async fn start_trading(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<StartTradingRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[start_trading] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    // Convert to instance::StartTradingRequest
    let instance_request = crate::onchain_instance::instance::StartTradingRequest {
        bucket_name: request.bucket_name,
        creator_pubkey: keypair.pubkey().to_string(),
    };
    match state.icm_client.start_trading_transaction(instance_request, keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[start_trading] Start trading error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            ResponseJson(error_response)
        }
    }
}

/// Swap tokens endpoint
#[axum::debug_handler]
pub async fn swap_tokens(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<SwapTokensRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[swap_tokens] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    // Convert to instance::SwapTokensRequest
    let instance_request = crate::onchain_instance::instance::SwapTokensRequest {
        bucket_pubkey: request.bucket,
        input_mint: request.input_mint,
        output_mint: request.output_mint,
        in_amount: request.in_amount,
        quoted_out_amount: request.quoted_out_amount,
        slippage_bps: request.slippage_bps,
        platform_fee_bps: request.platform_fee_bps,
        route_plan: request.route_plan,
        creator_pubkey: keypair.pubkey().to_string(),
    };
    match state.icm_client.swap_tokens_transaction(instance_request, keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[swap_tokens] Swap tokens error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            ResponseJson(error_response)
        }
    }
}

/// Claim rewards endpoint
#[axum::debug_handler]
pub async fn claim_rewards(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<ClaimRewardsRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[claim_rewards] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    // Convert to instance::ClaimRewardsRequest
    let instance_request = crate::onchain_instance::instance::ClaimRewardsRequest {
        bucket_name: request.bucket_name,
        token_mint: request.token_mint,
        contributor_pubkey: keypair.pubkey().to_string(),
    };
    match state.icm_client.claim_rewards_transaction(instance_request, keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[claim_rewards] Claim rewards error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            ResponseJson(error_response)
        }
    }
}

/// Close bucket endpoint
#[axum::debug_handler]
pub async fn close_bucket(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<CloseBucketRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[close_bucket] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    // Convert to instance::CloseBucketRequest
    let instance_request = crate::onchain_instance::instance::CloseBucketRequest {
        bucket_name: request.bucket_name,
        creator_pubkey: keypair.pubkey().to_string(),
    };
    match state.icm_client.close_bucket_transaction(instance_request, keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[close_bucket] Close bucket error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            ResponseJson(error_response)
        }
    }
}

/// Get bucket info endpoint (placeholder)
#[axum::debug_handler]
pub async fn get_bucket(
    State(_state): State<AppState>,
    Query(query): Query<GetBucketQuery>
) -> impl IntoResponse {
    // TODO: Implement actual bucket fetching
    let bucket_info = BucketInfo {
        public_key: String::new(),
        account: BucketAccount {
            creator: query.creator_pubkey,
            name: query.bucket_name,
            token_mints: vec![],
            contribution_deadline: String::new(),
            trading_deadline: String::new(),
            creator_fee_percent: 0,
            status: String::new(),
            total_contributions: String::new(),
            trading_started_at: String::new(),
            closed_at: String::new(),
            bump: 0,
        },
    };
    ResponseJson(ApiResponse::success(bucket_info))
}
