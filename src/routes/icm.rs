use std::str::FromStr;
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
    IcmProgramInstance,
    CreateBucketRequest as InstanceCreateBucketRequest,
    ContributeToBucketRequest as InstanceContributeToBucketRequest,
    StartTradingRequest as InstanceStartTradingRequest,
    SwapTokensRequest as InstanceSwapTokensRequest,
    ClaimRewardsRequest as InstanceClaimRewardsRequest,
    CloseBucketRequest as InstanceCloseBucketRequest,
    UnsignedTransactionResponse, GetBucketQuery, BucketInfo,
};


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
    use std::convert::TryFrom;
    Keypair::try_from(&privkey_bytes[..]).map_err(|_| "Invalid keypair bytes".to_string())
}

/// Get all pools (buckets) endpoint
#[axum::debug_handler]
pub async fn get_all_pools(
    State(state): State<AppState>,
) -> impl IntoResponse {
    use anchor_client::Program;
    use icm_program::accounts::Bucket as AnchorBucket;
    use crate::onchain_instance::instance::ICM_PROGRAM_ID;
    let cluster = state.icm_client.cluster.clone();
    let payer = solana_sdk::signature::Keypair::new();
    use solana_sdk::commitment_config::CommitmentConfig;
    let client = anchor_client::Client::new_with_options(cluster, std::sync::Arc::new(payer), CommitmentConfig::confirmed());
    let program = client.program(ICM_PROGRAM_ID).unwrap();

    // Fetch all bucket accounts
    let result = program.accounts::<AnchorBucket>(vec![]);
    let all_pools: Vec<BucketInfo> = match result.await {
        Ok(buckets) => buckets.into_iter().map(|(_pubkey, data)| {
            BucketInfo {
                name: data.name,
                creator_pubkey: data.creator.to_string(),
                token_mints: data.token_mints.iter().map(|k| k.to_string()).collect(),
                contribution_window_days: data.contribution_deadline as u32, // Cast i64 to u32
                trading_window_days: data.trading_deadline as u32, // Cast i64 to u32
                creator_fee_percent: data.creator_fee_percent,
            }
        }).collect(),
        Err(e) => {
            tracing::error!("Failed to fetch all pools: {}", e);
            vec![]
        }
    };
    ResponseJson(ApiResponse::success(all_pools))
}

/// Create bucket endpoint
#[axum::debug_handler]
pub async fn create_bucket(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<CreateBucketRequest>
) -> impl IntoResponse {
    tracing::info!("[create_bucket] Request received: user_id={}, email={}", auth_user.id, auth_user.email);
    tracing::info!("[create_bucket] Payload: name={}, token_mints={:?}, contribution_window_days={}, trading_window_days={}, creator_fee_percent={}", request.name, request.token_mints, request.contribution_window_days, request.trading_window_days, request.creator_fee_percent);
    let email = &auth_user.email;
    let keypair = match get_user_keypair_by_email(email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[create_bucket] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    let creator_pubkey = keypair.pubkey().to_string();
    tracing::info!("[create_bucket] Creator pubkey: {}", creator_pubkey);
    let instance_request = InstanceCreateBucketRequest {
        name: request.name.clone(),
        token_mints: request.token_mints.clone(),
        contribution_window_days: request.contribution_window_days,
        trading_window_days: request.trading_window_days,
        creator_fee_percent: request.creator_fee_percent,
        creator_pubkey,
    };
    match state.icm_client.create_bucket_transaction(instance_request, keypair).await {
        Ok(response) => {
            tracing::info!("[create_bucket] Transaction created successfully");
            ResponseJson(ApiResponse::success(response))
        },
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
    tracing::info!("[contribute_to_bucket] Request received: user_id={}, email={}", auth_user.id, auth_user.email);
    tracing::info!("[contribute_to_bucket] Payload: bucket_name={}, token_mint={}, amount={}", request.bucket_name, request.token_mint, request.amount);
    let email = &auth_user.email;
    let keypair = match get_user_keypair_by_email(email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[contribute_to_bucket] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    let contributor_pubkey = keypair.pubkey().to_string();
    tracing::info!("[contribute_to_bucket] Contributor pubkey: {}", contributor_pubkey);
    let instance_request = InstanceContributeToBucketRequest {
        bucket_name: request.bucket_name.clone(),
        token_mint: request.token_mint.clone(),
        amount: request.amount,
        contributor_pubkey,
    };
    match state.icm_client.contribute_to_bucket_transaction(instance_request, keypair).await {
        Ok(response) => {
            tracing::info!("[contribute_to_bucket] Transaction created successfully");
            ResponseJson(ApiResponse::success(response))
        },
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
    tracing::info!("[start_trading] Request received: user_id={}, email={}", auth_user.id, auth_user.email);
    tracing::info!("[start_trading] Payload: bucket_name={}", request.bucket_name);
    let email = &auth_user.email;
    let keypair = match get_user_keypair_by_email(email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[start_trading] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    let creator_pubkey = keypair.pubkey().to_string();
    tracing::info!("[start_trading] Creator pubkey: {}", creator_pubkey);
    let instance_request = InstanceStartTradingRequest {
        bucket_name: request.bucket_name.clone(),
        creator_pubkey,
    };
    match state.icm_client.start_trading_transaction(instance_request, keypair).await {
        Ok(response) => {
            tracing::info!("[start_trading] Transaction created successfully");
            ResponseJson(ApiResponse::success(response))
        },
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
    tracing::info!("[swap_tokens] Request received: user_id={}, email={}", auth_user.id, auth_user.email);
    tracing::info!("[swap_tokens] Payload: bucket={}, input_mint={}, output_mint={}, in_amount={}, quoted_out_amount={}, slippage_bps={}, platform_fee_bps={}, route_plan={:?}", request.bucket, request.input_mint, request.output_mint, request.in_amount, request.quoted_out_amount, request.slippage_bps, request.platform_fee_bps, request.route_plan);
    let email = &auth_user.email;
    let keypair = match get_user_keypair_by_email(email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[swap_tokens] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    let creator_pubkey = keypair.pubkey().to_string();
    tracing::info!("[swap_tokens] Creator pubkey: {}", creator_pubkey);
    let bucket_pubkey = Pubkey::from_str(&request.bucket).unwrap();
    tracing::info!("[swap_tokens] Bucket pubkey: {}", bucket_pubkey);
    let instance_request = InstanceSwapTokensRequest {
        creator_pubkey,
        bucket_pubkey: bucket_pubkey.to_string(),
        input_mint: request.input_mint.clone(),
        output_mint: request.output_mint.clone(),
        in_amount: request.in_amount,
        quoted_out_amount: request.quoted_out_amount,
        slippage_bps: request.slippage_bps,
        platform_fee_bps: request.platform_fee_bps,
        route_plan: request.route_plan.clone(),
    };
    match state.icm_client.swap_tokens_transaction(instance_request, keypair).await {
        Ok(response) => {
            tracing::info!("[swap_tokens] Transaction created successfully");
            ResponseJson(ApiResponse::success(response))
        },
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
    tracing::info!("[claim_rewards] Request received: user_id={}, email={}", auth_user.id, auth_user.email);
    tracing::info!("[claim_rewards] Payload: bucket_name={}, token_mint={}", request.bucket_name, request.token_mint);
    let email = &auth_user.email;
    let keypair = match get_user_keypair_by_email(email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[claim_rewards] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    let contributor_pubkey = keypair.pubkey().to_string();
    tracing::info!("[claim_rewards] Contributor pubkey: {}", contributor_pubkey);
    let instance_request = InstanceClaimRewardsRequest {
        bucket_name: request.bucket_name.clone(),
        token_mint: request.token_mint.clone(),
        contributor_pubkey,
    };
    match state.icm_client.claim_rewards_transaction(instance_request, keypair).await {
        Ok(response) => {
            tracing::info!("[claim_rewards] Transaction created successfully");
            ResponseJson(ApiResponse::success(response))
        },
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
    tracing::info!("[close_bucket] Request received: user_id={}, email={}", auth_user.id, auth_user.email);
    tracing::info!("[close_bucket] Payload: bucket_name={}", request.bucket_name);
    let email = &auth_user.email;
    let keypair = match get_user_keypair_by_email(email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[close_bucket] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    let creator_pubkey = keypair.pubkey().to_string();
    tracing::info!("[close_bucket] Creator pubkey: {}", creator_pubkey);
    let instance_request = InstanceCloseBucketRequest {
        bucket_name: request.bucket_name.clone(),
        creator_pubkey,
    };
    match state.icm_client.close_bucket_transaction(instance_request, keypair).await {
        Ok(response) => {
            tracing::info!("[close_bucket] Transaction created successfully");
            ResponseJson(ApiResponse::success(response))
        },
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
        name: query.bucket_name,
        creator_pubkey: query.creator_pubkey,
        token_mints: vec![],
        contribution_window_days: 0,
        trading_window_days: 0,
        creator_fee_percent: 0,
    };

    ResponseJson(ApiResponse::success(bucket_info))
}
