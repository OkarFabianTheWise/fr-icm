use axum::{Json, extract::{State, Extension}, response::IntoResponse};
use serde::{Serialize};
use crate::server::AppState;
use anchor_client::solana_sdk::signature::Keypair;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signer;
use axum::extract::Query;
use axum::response::Json as ResponseJson;
use anyhow::Result;
use anchor_lang::declare_program;
use chrono::{DateTime, Utc, Duration};
use uuid;

use crate::state_structs::{CreateBucketApiRequest, CreateBucketRequest, ContributeToBucketApiRequest, ContributeToBucketRequest,
UnsignedTransactionResponse, GetBucketQuery, BucketInfo, TradingPool, CloseBucketRequest, GetCreatorProfileQuery, ClaimRewardsRequest, StartTradingRequest, SwapTokensRequest, InitializeProgramRequest};

/// Convert human-readable USDC amount to lamports (multiply by 1e6)
fn usdc_to_lamports(usdc_amount: f64) -> u64 {
    (usdc_amount * 1_000_000.0) as u64
}

/// Convert lamports to human-readable USDC amount (divide by 1e6)
fn lamports_to_usdc(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000.0
}

/// Format seconds into a human-readable time string
fn format_time_remaining(seconds: i64) -> String {
    if seconds <= 0 {
        return "Ended".to_string();
    }
    
    let days = seconds / (24 * 3600);
    let hours = (seconds % (24 * 3600)) / 3600;
    let minutes = (seconds % 3600) / 60;
    
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 || parts.is_empty() {
        parts.push(format!("{}m", minutes));
    }
    
    parts.join(" ")
}

/// Calculate actual pool status and time remaining based on current time and deadlines
fn calculate_pool_status_and_time(
    stored_phase: &str,
    fundraising_deadline: i64,
    trading_start_time: Option<i64>,
    trading_end_time: Option<i64>,
    current_time: i64
) -> (String, Option<String>) {
    match stored_phase {
        "Raising" => {
            if current_time < fundraising_deadline {
                // Still in fundraising period
                let time_left = fundraising_deadline - current_time;
                ("Raising".to_string(), Some(format_time_remaining(time_left)))
            } else {
                // Fundraising deadline passed but start_trading not called yet
                ("Expired".to_string(), Some("Ended".to_string()))
            }
        },
        "Trading" => {
            if let Some(end_time) = trading_end_time {
                if current_time < end_time {
                    // Still in trading period
                    let time_left = end_time - current_time;
                    ("Trading".to_string(), Some(format_time_remaining(time_left)))
                } else {
                    // Trading period ended
                    ("Closed".to_string(), Some("Ended".to_string()))
                }
            } else {
                // Trading started but no end time set
                ("Trading".to_string(), None)
            }
        },
        _ => {
            // Closed, Failed, etc.
            (stored_phase.to_string(), Some("Ended".to_string()))
        }
    }
}

use std::convert::TryFrom;
use std::str::FromStr;
use std::collections::HashMap;
use spl_token_2022::ID as TOKEN_2022_PROGRAM_ID;
declare_program!(icm_program);
const ICM_PROGRAM_ID: Pubkey = icm_program::ID;


// Conversion from icm_program::accounts::TradingPool to local TradingPool
impl From<icm_program::accounts::TradingPool> for TradingPool {
    fn from(src: icm_program::accounts::TradingPool) -> Self {
        TradingPool {
            pool_id: src.pool_id.to_string(),
            pool_bump: src.pool_bump,
            creator: src.creator.to_string(),
            token_bucket: src.token_bucket.into_iter().map(|pk| pk.to_string()).collect(),
            target_amount: src.target_amount.to_string(),
            min_contribution: src.min_contribution.to_string(),
            max_contribution: src.max_contribution.to_string(),
            trading_duration: src.trading_duration.to_string(),
            created_at: src.created_at.to_string(),
            fundraising_deadline: src.fundraising_deadline.to_string(),
            trading_start_time: src.trading_start_time.map(|v| v.to_string()),
            trading_end_time: src.trading_end_time.map(|v| v.to_string()),
            phase: format!("{:?}", src.phase),
            management_fee: src.management_fee,
            raised_amount: None,
            contribution_percent: None,
            strategy: None,
            time_remaining: None,
        }
    }
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
pub async fn get_user_keypair_by_email(email: &str, state: &AppState) -> Result<Keypair, String> {
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

/// Check if program is initialized endpoint
#[axum::debug_handler]
pub async fn check_program_status(
    State(state): State<AppState>,
    Query(query): Query<std::collections::HashMap<String, String>>
) -> ResponseJson<ApiResponse<serde_json::Value>> {
    let usdc_mint_str = match query.get("usdc_mint") {
        Some(mint) => mint,
        None => {
            return ResponseJson(ApiResponse::error("usdc_mint parameter is required".to_string()));
        }
    };

    let usdc_mint = match Pubkey::from_str(usdc_mint_str) {
        Ok(mint) => mint,
        Err(_) => {
            return ResponseJson(ApiResponse::error("Invalid usdc_mint format".to_string()));
        }
    };

    match state.icm_client.check_program_initialized(usdc_mint).await {
        Ok(is_initialized) => {
            let response = serde_json::json!({
                "initialized": is_initialized,
                "message": if is_initialized { 
                    "Program is initialized and ready to use" 
                } else { 
                    "Program is not initialized. Please call /api/v1/program/initialize first" 
                }
            });
            ResponseJson(ApiResponse::success(response))
        },
        Err(e) => ResponseJson(ApiResponse::error(format!("Failed to check program status: {}", e))),
    }
}

/// Initialize program endpoint (must be called first before any other operations)
#[axum::debug_handler]
pub async fn initialize_program(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<InitializeProgramRequest>
) -> ResponseJson<ApiResponse<UnsignedTransactionResponse>> {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(keypair) => keypair,
        Err(e) => {
            return ResponseJson(ApiResponse::error(format!("Failed to get keypair: {}", e)));
        }
    };

    // Call the initialize program transaction
    let result = state.icm_client.initialize_program_transaction(request, keypair).await;

    match result {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => ResponseJson(ApiResponse::error(format!("Failed to initialize program: {}", e))),
    }
}

/// Create profile endpoint
#[axum::debug_handler]
pub async fn create_profile(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>
) -> ResponseJson<ApiResponse<UnsignedTransactionResponse>> {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            // tracing::error!("[create_profile] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    match state.icm_client.create_profile_transaction(keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[create_profile] Create profile error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            ResponseJson(error_response)
        }
    }
}

/// Get all pools (buckets) endpoint
#[axum::debug_handler]
pub async fn get_trading_pool(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Query(query): Query<GetBucketQuery>,
) -> ResponseJson<ApiResponse<TradingPool>> {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            // tracing::error!("[get_trading_pool] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<TradingPool>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    let raised_amount = query.raised_amount.clone();
    let creator_pubkey = query.creator_pubkey.clone();
    let bucket_name = query.bucket_name.clone();
    match state.icm_client.fetch_trading_pool_by_pda(query, keypair).await {
        Ok(pool) => {
            // Calculate contribution_percent if raised_amount is provided
            let (contribution_percent, raised_amount_str) = if let Some(raised) = raised_amount {
                // Convert lamports to USDC for display and calculation
                let raised_lamports = raised.parse::<u64>().unwrap_or(0);
                let target_lamports = pool.target_amount.parse::<u64>().unwrap_or(0);
                let raised_usdc = lamports_to_usdc(raised_lamports);
                let target_usdc = lamports_to_usdc(target_lamports);
                let percent = if target_usdc > 0.0 { (raised_usdc / target_usdc) * 100.0 } else { 0.0 };
                (Some(percent), Some(format!("{:.2}", raised_usdc)))
            } else {
                (None, None)
            };

            // Fetch strategy from database
            tracing::debug!("[get_trading_pool] Fetching strategy for creator: {}, bucket: {}", creator_pubkey, bucket_name);
            let strategy = match crate::database::models::DatabaseTradingPool::fetch_pool_strategy_by_creator_and_name(
                state.db.pool(),
                &creator_pubkey,
                &bucket_name,
            ).await {
                Ok(strategy_opt) => {
                    tracing::debug!("[get_trading_pool] Strategy fetch result: {:?}", strategy_opt);
                    strategy_opt
                },
                Err(e) => {
                    tracing::warn!("[get_trading_pool] Failed to fetch strategy from database: {}", e);
                    None
                }
            };

            // Calculate actual pool status and time remaining based on current time and deadlines
            let current_time = chrono::Utc::now().timestamp();
            let fundraising_deadline = pool.fundraising_deadline.parse::<i64>().unwrap_or(0);
            let trading_start = pool.trading_start_time.as_ref().and_then(|t| t.parse::<i64>().ok());
            let trading_end = pool.trading_end_time.as_ref().and_then(|t| t.parse::<i64>().ok());
            
            let (computed_phase, time_remaining) = calculate_pool_status_and_time(
                &pool.phase,
                fundraising_deadline,
                trading_start,
                trading_end,
                current_time
            );

            let mapped = TradingPool {
                pool_id: pool.pool_id,
                pool_bump: pool.pool_bump,
                creator: pool.creator,
                token_bucket: pool.token_bucket,
                target_amount: format!("{:.2}", lamports_to_usdc(pool.target_amount.parse::<u64>().unwrap_or(0))),
                min_contribution: format!("{:.2}", lamports_to_usdc(pool.min_contribution.parse::<u64>().unwrap_or(0))),
                max_contribution: format!("{:.2}", lamports_to_usdc(pool.max_contribution.parse::<u64>().unwrap_or(0))),
                trading_duration: pool.trading_duration,
                created_at: pool.created_at,
                fundraising_deadline: pool.fundraising_deadline,
                trading_start_time: pool.trading_start_time,
                trading_end_time: pool.trading_end_time,
                phase: computed_phase, // Use computed phase instead of raw phase
                management_fee: pool.management_fee,
                raised_amount: raised_amount_str,
                contribution_percent,
                strategy,
                time_remaining, // Include calculated time remaining
            };
            ResponseJson(ApiResponse::success(mapped))
        },
        Err(e) => {
            tracing::error!("Failed to fetch trading pool: {}", e);
            ResponseJson(ApiResponse::<TradingPool>::error(e.to_string()))
        }
    }
}

#[axum::debug_handler]
pub async fn get_all_pools_by_pda(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>
) -> ResponseJson<ApiResponse<Vec<BucketInfo>>> {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            // tracing::error!("[get_all_pools] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<Vec<BucketInfo>>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    match state.icm_client.get_all_pools_by_pda(keypair, state.db.pool()).await {
        Ok(all_pools) => ResponseJson(ApiResponse::success(all_pools)),
        Err(e) => {
            // tracing::error!("Failed to fetch all pools: {}", e);
            ResponseJson(ApiResponse::<Vec<BucketInfo>>::error(e.to_string()))
        }
    }
}

/// Get trading pool info endpoint
#[axum::debug_handler]
pub async fn get_trading_pool_info(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Query(query): Query<GetBucketQuery>
) -> ResponseJson<ApiResponse<TradingPool>> {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[get_trading_pool_info] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<TradingPool>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };

    // Log query parameters
    tracing::debug!("[get_trading_pool_info] Query parameters: {:?}", query);

    match state.icm_client.fetch_trading_pool_by_pda(query.clone(), keypair).await {
        Ok(pool_info) => {
            // Calculate contribution_percent if raised_amount is provided
            let (contribution_percent, raised_amount_str) = if let Some(raised) = query.raised_amount.clone() {
                // Convert lamports to USDC for display and calculation
                let raised_lamports = raised.parse::<u64>().unwrap_or(0);
                let target_lamports = pool_info.target_amount.parse::<u64>().unwrap_or(0);
                let raised_usdc = lamports_to_usdc(raised_lamports);
                let target_usdc = lamports_to_usdc(target_lamports);
                let percent = if target_usdc > 0.0 { (raised_usdc / target_usdc) * 100.0 } else { 0.0 };
                (Some(percent), Some(format!("{:.2}", raised_usdc)))
            } else {
                (None, None)
            };

            // Fetch strategy from database
            tracing::debug!("[get_trading_pool_info] Fetching strategy for creator: {}, bucket: {}", query.creator_pubkey, query.bucket_name);
            let strategy = match crate::database::models::DatabaseTradingPool::fetch_pool_strategy_by_creator_and_name(
                state.db.pool(),
                &query.creator_pubkey,
                &query.bucket_name,
            ).await {
                Ok(strategy_opt) => {
                    tracing::debug!("[get_trading_pool_info] Strategy fetch result: {:?}", strategy_opt);
                    strategy_opt
                },
                Err(e) => {
                    tracing::warn!("[get_trading_pool_info] Failed to fetch strategy from database: {}", e);
                    None
                }
            };

            // Calculate actual pool status and time remaining based on current time and deadlines
            let current_time = chrono::Utc::now().timestamp();
            let fundraising_deadline = pool_info.fundraising_deadline.parse::<i64>().unwrap_or(0);
            let trading_start = pool_info.trading_start_time.as_ref().and_then(|t| t.parse::<i64>().ok());
            let trading_end = pool_info.trading_end_time.as_ref().and_then(|t| t.parse::<i64>().ok());
            
            let (computed_phase, time_remaining) = calculate_pool_status_and_time(
                &pool_info.phase,
                fundraising_deadline,
                trading_start,
                trading_end,
                current_time
            );

            let mapped = TradingPool {
                pool_id: pool_info.pool_id,
                pool_bump: pool_info.pool_bump,
                creator: pool_info.creator,
                token_bucket: pool_info.token_bucket,
                target_amount: format!("{:.2}", lamports_to_usdc(pool_info.target_amount.parse::<u64>().unwrap_or(0))),
                min_contribution: format!("{:.2}", lamports_to_usdc(pool_info.min_contribution.parse::<u64>().unwrap_or(0))),
                max_contribution: format!("{:.2}", lamports_to_usdc(pool_info.max_contribution.parse::<u64>().unwrap_or(0))),
                trading_duration: pool_info.trading_duration,
                created_at: pool_info.created_at,
                fundraising_deadline: pool_info.fundraising_deadline,
                trading_start_time: pool_info.trading_start_time,
                trading_end_time: pool_info.trading_end_time,
                phase: computed_phase, // Use computed phase instead of raw phase
                management_fee: pool_info.management_fee,
                raised_amount: raised_amount_str,
                contribution_percent,
                strategy,
                time_remaining, // Include calculated time remaining
            };
            ResponseJson(ApiResponse::success(mapped))
        },
        Err(e) => {
            tracing::error!("[get_trading_pool_info] Error fetching trading pool info: {}", e);
            ResponseJson(ApiResponse::<TradingPool>::error(e.to_string()))
        }
    }
}

/// Create bucket endpoint
#[axum::debug_handler]
pub async fn create_bucket(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<CreateBucketApiRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            // tracing::error!("[create_bucket] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };

    // Check if creator profile exists, create if not
    let creator_query = GetCreatorProfileQuery {
        creator_pubkey: keypair.pubkey().to_string(),
    };
    
    match state.icm_client.fetch_creator_profile_by_pda(creator_query, keypair.insecure_clone()).await {
        Ok(_) => {
            tracing::info!("[create_bucket] Creator profile exists, proceeding with bucket creation");
        },
        Err(e) => {
            let error_str = e.to_string();
            // Only try to create if it's actually missing (not other errors)
            if error_str.contains("Account does not exist") || error_str.contains("AccountNotFound") {
                // tracing::info!("[create_bucket] Creator profile doesn't exist, creating it first");
                match state.icm_client.create_profile_transaction(keypair.insecure_clone()).await {
                    Ok(profile_response) => {
                        tracing::info!("[create_bucket] Creator profile created: {}", profile_response.transaction);
                    },
                    Err(create_err) => {
                        let create_error_str = create_err.to_string();
                        // If it's "already in use", the profile actually exists, so continue
                        if create_error_str.contains("already in use") || create_error_str.contains("custom program error: 0x0") {
                            tracing::info!("[create_bucket] Creator profile already exists (race condition), continuing");
                            // return result with error

                        } else {
                            // tracing::error!("[create_bucket] Failed to create creator profile: {}", create_err);
                            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(
                                format!("Failed to create creator profile: {}", create_err)
                            );
                            return ResponseJson(error_response);
                        }
                    }
                }
            } else {
                tracing::info!("[create_bucket] Creator profile fetch failed with non-missing error, assuming it exists: {}", error_str);
            }
        }
    }

    // Convert to instance::CreateBucketRequest (convert human-readable USDC to lamports)
    let instance_request = CreateBucketRequest {
        name: request.name.clone(),
        token_mints: request.token_mints.clone(),
        contribution_window_minutes: request.contribution_window_minutes,
        trading_window_minutes: request.trading_window_minutes,
        creator_fee_percent: request.creator_fee_percent,
        target_amount: usdc_to_lamports(request.target_amount),
        min_contribution: usdc_to_lamports(request.min_contribution),
        max_contribution: usdc_to_lamports(request.max_contribution),
        management_fee: request.management_fee,
        strategy: request.strategy.clone(),
    };
    match state.icm_client.create_bucket_transaction(instance_request, keypair.insecure_clone()).await {
        Ok(response) => {
            // Save pool information to database after successful blockchain transaction
            let creator_pubkey = keypair.pubkey().to_string();
            let trading_end_time = chrono::Utc::now() + chrono::Duration::minutes(request.trading_window_minutes as i64);
            
            tracing::debug!("[create_bucket] Saving pool to database - creator: {}, name: {}, strategy: {}", 
                creator_pubkey, request.name, request.strategy);
            
            if let Err(db_err) = crate::database::models::DatabaseTradingPool::insert_trading_pool(
                state.db.pool(),
                &creator_pubkey,
                &request.name,
                &request.strategy,
                request.token_mints.clone(),
                usdc_to_lamports(request.target_amount) as i64,
                trading_end_time,
                request.management_fee as i32,
            ).await {
                tracing::warn!("[create_bucket] Failed to save pool to database: {}", db_err);
            } else {
                tracing::info!("[create_bucket] Pool saved to database successfully - creator: {}, name: {}, strategy: {}", 
                    creator_pubkey, request.name, request.strategy);
            }
            
            ResponseJson(ApiResponse::success(response))
        },
        Err(e) => {
            // tracing::error!("[create_bucket] Create bucket error: {}", e);
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
    Json(request): Json<ContributeToBucketApiRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            // tracing::error!("[contribute_to_bucket] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    // Convert to instance::ContributeToBucketRequest (convert human-readable USDC to lamports)
    let instance_request = ContributeToBucketRequest {
        bucket_name: request.bucket_name,
        amount: usdc_to_lamports(request.amount),
        creator_pubkey: request.creator_pubkey.clone(),
    };
    match state.icm_client.contribute_to_bucket_transaction(instance_request, keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            // tracing::error!("[contribute_to_bucket] Contribute to bucket error: {}", e);
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

    // Generate a unique pool ID
    let pool_id = uuid::Uuid::new_v4().to_string();
    
    // Save trading pool info to DB using the standardized function
    let trading_end_time = chrono::DateTime::parse_from_rfc3339(&request.trading_end_time)
        .map_err(|e| format!("Invalid trading_end_time format: {}", e))
        .and_then(|dt| Ok(dt.with_timezone(&chrono::Utc)))
        .unwrap_or_else(|_| chrono::Utc::now() + chrono::Duration::days(30)); // Default to 30 days if parsing fails
    
    if let Err(e) = crate::database::models::DatabaseTradingPool::insert_trading_pool(
        state.db.pool(),
        &request.creator_pubkey,
        &request.pool_name,
        &request.strategy,
        request.token_bucket.clone(),
        request.total_amount_available_to_trade,
        trading_end_time,
        request.management_fee,
    ).await {
        tracing::error!("[start_trading] Failed to save pool to database: {}", e);
        let error_response = ApiResponse::<UnsignedTransactionResponse>::error("Failed to save pool to database".to_string());
        return ResponseJson(error_response);
    }

    // Derive token pairs from token_bucket (all unique pairs)
    let mut token_pairs = Vec::new();
    let tokens = &request.token_bucket;
    for i in 0..tokens.len() {
        for j in 0..tokens.len() {
            if i != j {
                token_pairs.push((tokens[i].clone(), tokens[j].clone()));
            }
        }
    }

    // Build a default StrategyConfig from the strategy string
    let strategy_type = match request.strategy.to_lowercase().as_str() {
        "arbitrage" => crate::agent::types::StrategyType::Arbitrage,
        "gridtrading" | "grid_trading" => crate::agent::types::StrategyType::GridTrading,
        "dca" => crate::agent::types::StrategyType::DCA,
        "meanreversion" | "mean_reversion" => crate::agent::types::StrategyType::MeanReversion,
        "trendfollowing" | "trend_following" => crate::agent::types::StrategyType::TrendFollowing,
        _ => crate::agent::types::StrategyType::DCA,
    };
    let strategy_config = crate::agent::types::StrategyConfig {
        strategy_type,
        parameters: crate::agent::types::StrategyParameters {
            min_spread_bps: 10,
            max_slippage_bps: 50,
            position_size_usd: 100.0,
            rebalance_threshold_pct: 5.0,
            lookback_periods: 10,
            custom_params: std::collections::HashMap::new(),
        },
        risk_limits: crate::agent::types::RiskLimits {
            max_position_size_usd: 1000.0,
            max_daily_loss_pct: 10.0,
            max_drawdown_pct: 20.0,
            stop_loss_pct: 5.0,
            take_profit_pct: 10.0,
        },
        execution_settings: crate::agent::types::ExecutionSettings {
            priority_fee_percentile: 90,
            max_priority_fee_lamports: 10000,
            transaction_timeout_ms: 60000,
            retry_attempts: 3,
            jito_tip_lamports: 0,
        },
    };

    // Get OpenAI API key from environment
    let openai_api_key = std::env::var("OPENAI_API_KEY")
        .unwrap_or_else(|_| {
            tracing::warn!("[start_trading] OPENAI_API_KEY not set, using default");
            "sk-proj-default".to_string()
        });

    // Call the actual start_trading_transaction to create the blockchain transaction
    tracing::info!("[start_trading] Calling start_trading_transaction to create blockchain transaction");
    let tx_response = match state.icm_client.start_trading_transaction(request.clone(), keypair).await {
        Ok(response) => {
            tracing::info!("[start_trading] Blockchain transaction created successfully: {}", response.transaction);
            response
        },
        Err(e) => {
            tracing::error!("[start_trading] Failed to create blockchain transaction: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(format!("Failed to create blockchain transaction: {}", e));
            return ResponseJson(error_response);
        }
    };

    let agent_config = crate::agent::trading_agent::TradingAgentConfigBuilder::new()
        .with_openai_api_key(openai_api_key)
        .with_token_pairs(token_pairs)
        .with_strategy_configs(vec![strategy_config])
        .with_portfolio_id(uuid::Uuid::parse_str(&pool_id).unwrap())
        .build();
    match agent_config {
        Ok(config) => {
            let icm_client = state.icm_client.clone();
            let db_pool = state.db.pool().clone();
            tokio::spawn(async move {
                match crate::agent::trading_agent::TradingAgent::new(config, icm_client, db_pool).await {
                    Ok(agent) => {
                        tracing::info!("[start_trading] Trading agent created successfully");
                    },
                    Err(e) => {
                        tracing::error!("[start_trading] Failed to create trading agent: {}", e);
                    }
                }
            });
        },
        Err(e) => {
            tracing::error!("[start_trading] Invalid agent config: {}", e);
        }
    }

    ResponseJson(ApiResponse::success(tx_response))
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
    let instance_request = SwapTokensRequest {
        bucket: request.bucket.clone(),
        input_mint: request.input_mint.clone(),
        output_mint: request.output_mint.clone(),
        in_amount: request.in_amount,
        quoted_out_amount: request.quoted_out_amount,
        slippage_bps: request.slippage_bps,
        amm: request.amm.clone(),
        amm_authority: request.amm_authority.clone(),
        pool_coin_token_account: request.pool_coin_token_account.clone(),
        pool_pc_token_account: request.pool_pc_token_account.clone(),
    };
    // Parse required pubkeys
    let bucket_name = &request.bucket;
    let input_mint = Pubkey::from_str(&request.input_mint).unwrap();
    let output_mint = Pubkey::from_str(&request.output_mint).unwrap();

    // Get Raydium AMM parameters from environment or request
    let raydium_amm_program = Pubkey::from_str(&std::env::var("RAYDIUM_AMM_PROGRAM").unwrap_or_else(|_| "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string())).expect("Invalid RAYDIUM_AMM_PROGRAM");
    
    // Parse AMM accounts from request or use defaults/environment
    let amm = request.amm.as_ref()
        .map(|s| Pubkey::from_str(s).expect("Invalid AMM address"))
        .unwrap_or_else(|| Pubkey::from_str(&std::env::var("DEFAULT_AMM").unwrap_or_default()).expect("Invalid DEFAULT_AMM"));
    
    let amm_authority = request.amm_authority.as_ref()
        .map(|s| Pubkey::from_str(s).expect("Invalid AMM authority"))
        .unwrap_or_else(|| Pubkey::from_str(&std::env::var("DEFAULT_AMM_AUTHORITY").unwrap_or_default()).expect("Invalid DEFAULT_AMM_AUTHORITY"));
    
    let pool_coin_token_account = request.pool_coin_token_account.as_ref()
        .map(|s| Pubkey::from_str(s).expect("Invalid pool coin token account"))
        .unwrap_or_else(|| Pubkey::from_str(&std::env::var("DEFAULT_POOL_COIN_TOKEN_ACCOUNT").unwrap_or_default()).expect("Invalid DEFAULT_POOL_COIN_TOKEN_ACCOUNT"));
    
    let pool_pc_token_account = request.pool_pc_token_account.as_ref()
        .map(|s| Pubkey::from_str(s).expect("Invalid pool pc token account"))
        .unwrap_or_else(|| Pubkey::from_str(&std::env::var("DEFAULT_POOL_PC_TOKEN_ACCOUNT").unwrap_or_default()).expect("Invalid DEFAULT_POOL_PC_TOKEN_ACCOUNT"));

    // For user_authority, use the bucket authority (derived from bucket)
    let creator = keypair.pubkey();
    let (bucket_pda, _) = Pubkey::find_program_address(
        &[b"bucket", bucket_name.as_bytes(), creator.as_ref()],
        &crate::onchain_instance::instance::ICM_PROGRAM_ID,
    );
    let user_authority = bucket_pda; // The bucket PDA acts as the user authority

    match state.icm_client.agent_swap_tokens_transaction(
        instance_request,
        keypair,
        bucket_name,
        input_mint,
        output_mint,
        raydium_amm_program,
        amm,
        amm_authority,
        pool_coin_token_account,
        pool_pc_token_account,
        user_authority,
    ).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[swap_tokens] Agent swap tokens error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            ResponseJson(error_response)
        }
    }
}

/// Agent swap tokens endpoint
#[axum::debug_handler]
pub async fn agent_swap_tokens(
    State(state): State<AppState>,
    Extension(auth_user): Extension<crate::auth::models::AuthUser>,
    Json(request): Json<SwapTokensRequest>
) -> impl IntoResponse {
    // Get user keypair
    let keypair = match get_user_keypair_by_email(&auth_user.email, &state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("[agent_swap_tokens] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    // Convert to instance::SwapTokensRequest
    let instance_request = SwapTokensRequest {
        bucket: request.bucket.clone(),
        input_mint: request.input_mint.clone(),
        output_mint: request.output_mint.clone(),
        in_amount: request.in_amount,
        quoted_out_amount: request.quoted_out_amount,
        slippage_bps: request.slippage_bps,
        amm: request.amm.clone(),
        amm_authority: request.amm_authority.clone(),
        pool_coin_token_account: request.pool_coin_token_account.clone(),
        pool_pc_token_account: request.pool_pc_token_account.clone(),
    };
    // Parse required pubkeys
    let bucket_name = &request.bucket;
    let input_mint = Pubkey::from_str(&request.input_mint).unwrap();
    let output_mint = Pubkey::from_str(&request.output_mint).unwrap();

    // Get Raydium AMM parameters from environment or request
    let raydium_amm_program = Pubkey::from_str(&std::env::var("RAYDIUM_AMM_PROGRAM").unwrap_or_else(|_| "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string())).expect("Invalid RAYDIUM_AMM_PROGRAM");
    
    // Parse AMM accounts from request or use defaults/environment
    let amm = request.amm.as_ref()
        .map(|s| Pubkey::from_str(s).expect("Invalid AMM address"))
        .unwrap_or_else(|| Pubkey::from_str(&std::env::var("DEFAULT_AMM").unwrap_or_default()).expect("Invalid DEFAULT_AMM"));
    
    let amm_authority = request.amm_authority.as_ref()
        .map(|s| Pubkey::from_str(s).expect("Invalid AMM authority"))
        .unwrap_or_else(|| Pubkey::from_str(&std::env::var("DEFAULT_AMM_AUTHORITY").unwrap_or_default()).expect("Invalid DEFAULT_AMM_AUTHORITY"));
    
    let pool_coin_token_account = request.pool_coin_token_account.as_ref()
        .map(|s| Pubkey::from_str(s).expect("Invalid pool coin token account"))
        .unwrap_or_else(|| Pubkey::from_str(&std::env::var("DEFAULT_POOL_COIN_TOKEN_ACCOUNT").unwrap_or_default()).expect("Invalid DEFAULT_POOL_COIN_TOKEN_ACCOUNT"));
    
    let pool_pc_token_account = request.pool_pc_token_account.as_ref()
        .map(|s| Pubkey::from_str(s).expect("Invalid pool pc token account"))
        .unwrap_or_else(|| Pubkey::from_str(&std::env::var("DEFAULT_POOL_PC_TOKEN_ACCOUNT").unwrap_or_default()).expect("Invalid DEFAULT_POOL_PC_TOKEN_ACCOUNT"));

    // For user_authority, use the bucket authority (derived from bucket)
    let creator = keypair.pubkey();
    let (bucket_pda, _) = Pubkey::find_program_address(
        &[b"bucket", bucket_name.as_bytes(), creator.as_ref()],
        &crate::onchain_instance::instance::ICM_PROGRAM_ID,
    );
    let user_authority = bucket_pda; // The bucket PDA acts as the user authority

    match state.icm_client.agent_swap_tokens_transaction(
        instance_request,
        keypair,
        bucket_name,
        input_mint,
        output_mint,
        raydium_amm_program,
        amm,
        amm_authority,
        pool_coin_token_account,
        pool_pc_token_account,
        user_authority,
    ).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
        Err(e) => {
            tracing::error!("[agent_swap_tokens] Agent swap tokens error: {}", e);
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
    let instance_request = ClaimRewardsRequest {
        bucket_name: request.bucket_name,
        token_mint: request.token_mint,
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
    let instance_request = CloseBucketRequest {
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

