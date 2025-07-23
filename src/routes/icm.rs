use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::Json as ResponseJson,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use anyhow::Result;

use crate::onchain_instance::instance::{
    IcmProgramInstance, CreateBucketRequest, ContributeToBucketRequest,
    StartTradingRequest, SwapTokensRequest, ClaimRewardsRequest, CloseBucketRequest,
    UnsignedTransactionResponse, GetBucketQuery, BucketInfo,
};
use crate::server::AppState;

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

/// Create bucket endpoint
pub async fn create_bucket(
    State(state): State<AppState>,
    Json(request): Json<CreateBucketRequest>
) -> Result<ResponseJson<ApiResponse<UnsignedTransactionResponse>>, StatusCode> {
    match state.icm_client.create_bucket_transaction(request).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(e) => {
            eprintln!("Create bucket error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Contribute to bucket endpoint
pub async fn contribute_to_bucket(
    State(state): State<AppState>,
    Json(request): Json<ContributeToBucketRequest>
) -> Result<ResponseJson<ApiResponse<UnsignedTransactionResponse>>, StatusCode> {
    match state.icm_client.contribute_to_bucket_transaction(request).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(e) => {
            eprintln!("Contribute to bucket error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Start trading endpoint
pub async fn start_trading(
    State(state): State<AppState>,
    Json(request): Json<StartTradingRequest>
) -> Result<ResponseJson<ApiResponse<UnsignedTransactionResponse>>, StatusCode> {
    match state.icm_client.start_trading_transaction(request).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(e) => {
            eprintln!("Start trading error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Swap tokens endpoint
pub async fn swap_tokens(
    State(state): State<AppState>,
    Json(request): Json<SwapTokensRequest>
) -> Result<ResponseJson<ApiResponse<UnsignedTransactionResponse>>, StatusCode> {
    match state.icm_client.swap_tokens_transaction(request).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(e) => {
            eprintln!("Swap tokens error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Claim rewards endpoint
pub async fn claim_rewards(
    State(state): State<AppState>,
    Json(request): Json<ClaimRewardsRequest>
) -> Result<ResponseJson<ApiResponse<UnsignedTransactionResponse>>, StatusCode> {
    match state.icm_client.claim_rewards_transaction(request).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(e) => {
            eprintln!("Claim rewards error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Close bucket endpoint
pub async fn close_bucket(
    State(state): State<AppState>,
    Json(request): Json<CloseBucketRequest>
) -> Result<ResponseJson<ApiResponse<UnsignedTransactionResponse>>, StatusCode> {
    match state.icm_client.close_bucket_transaction(request).await {
        Ok(response) => Ok(ResponseJson(ApiResponse::success(response))),
        Err(e) => {
            eprintln!("Close bucket error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get bucket info endpoint (placeholder)
pub async fn get_bucket(
    State(_state): State<AppState>,
    Query(query): Query<GetBucketQuery>
) -> Result<ResponseJson<ApiResponse<BucketInfo>>, StatusCode> {
    // TODO: Implement actual bucket fetching
    let bucket_info = BucketInfo {
        name: query.name,
        creator: query.creator,
        status: "Raising".to_string(),
        contribution_deadline: 0,
        trading_deadline: 0,
    };

    Ok(ResponseJson(ApiResponse::success(bucket_info)))
}
