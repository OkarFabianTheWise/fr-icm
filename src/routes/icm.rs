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

use crate::state_structs::{CreateBucketRequest,
UnsignedTransactionResponse, GetBucketQuery, BucketInfo, TradingPool, CloseBucketRequest, GetCreatorProfileQuery, ContributeToBucketRequest, ClaimRewardsRequest, StartTradingRequest, SwapTokensRequest};

use std::convert::TryFrom;
use std::str::FromStr;
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
    match state.icm_client.fetch_trading_pool_by_pda(query, keypair).await {
        Ok(pool) => {
            // Calculate contribution_percent if raised_amount is provided
            let (contribution_percent, raised_amount_str) = if let Some(raised) = raised_amount {
                // Both raised and target_amount are strings, try to parse as f64
                let raised_f = raised.parse::<f64>().unwrap_or(0.0);
                let target_f = pool.target_amount.parse::<f64>().unwrap_or(0.0);
                let percent = if target_f > 0.0 { (raised_f / target_f) * 100.0 } else { 0.0 };
                (Some(percent), Some(raised))
            } else {
                (None, None)
            };
            let mapped = TradingPool {
                pool_id: pool.pool_id,
                pool_bump: pool.pool_bump,
                creator: pool.creator,
                token_bucket: pool.token_bucket,
                target_amount: pool.target_amount,
                min_contribution: pool.min_contribution,
                max_contribution: pool.max_contribution,
                trading_duration: pool.trading_duration,
                created_at: pool.created_at,
                fundraising_deadline: pool.fundraising_deadline,
                trading_start_time: pool.trading_start_time,
                trading_end_time: pool.trading_end_time,
                phase: pool.phase,
                management_fee: pool.management_fee,
                raised_amount: raised_amount_str,
                contribution_percent,
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
    match state.icm_client.get_all_pools_by_pda(keypair).await {
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
            // tracing::error!("[get_trading_pool_info] Failed to get user keypair: {}", e);
            let error_response = ApiResponse::<TradingPool>::error(e.to_string());
            return ResponseJson(error_response);
        }
    };
    match state.icm_client.fetch_trading_pool_by_pda(query.clone(), keypair).await {
        Ok(pool_info) => {
            // Calculate contribution_percent if raised_amount is provided
            let (contribution_percent, raised_amount_str) = if let Some(raised) = query.raised_amount.clone() {
                let raised_f = raised.parse::<f64>().unwrap_or(0.0);
                let target_f = pool_info.target_amount.parse::<f64>().unwrap_or(0.0);
                let percent = if target_f > 0.0 { (raised_f / target_f) * 100.0 } else { 0.0 };
                (Some(percent), Some(raised))
            } else {
                (None, None)
            };
            let mapped = TradingPool {
                pool_id: pool_info.pool_id,
                pool_bump: pool_info.pool_bump,
                creator: pool_info.creator,
                token_bucket: pool_info.token_bucket,
                target_amount: pool_info.target_amount,
                min_contribution: pool_info.min_contribution,
                max_contribution: pool_info.max_contribution,
                trading_duration: pool_info.trading_duration,
                created_at: pool_info.created_at,
                fundraising_deadline: pool_info.fundraising_deadline,
                trading_start_time: pool_info.trading_start_time,
                trading_end_time: pool_info.trading_end_time,
                phase: pool_info.phase,
                management_fee: pool_info.management_fee,
                raised_amount: raised_amount_str,
                contribution_percent,
            };
            ResponseJson(ApiResponse::success(mapped))
        },
        Err(e) => {
            // tracing::error!("[get_trading_pool_info] Error fetching trading pool info: {}", e);
            ResponseJson(ApiResponse::<TradingPool>::error(e.to_string()))
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

    // Convert to instance::CreateBucketRequest
    let instance_request = CreateBucketRequest {
        name: request.name,
        token_mints: request.token_mints,
        contribution_window_days: request.contribution_window_days,
        trading_window_days: request.trading_window_days,
        creator_fee_percent: request.creator_fee_percent,
        target_amount: request.target_amount,
        min_contribution: request.min_contribution,
        max_contribution: request.max_contribution,
        management_fee: request.management_fee,
    };
    match state.icm_client.create_bucket_transaction(instance_request, keypair).await {
        Ok(response) => ResponseJson(ApiResponse::success(response)),
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
    Json(request): Json<ContributeToBucketRequest>
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
    // Convert to instance::ContributeToBucketRequest
    let instance_request = ContributeToBucketRequest {
        bucket_name: request.bucket_name,
        amount: request.amount,
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

    // Save trading pool info to DB
    let pool = state.db.pool();
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[start_trading] DB connection error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error("DB connection error".to_string());
            return ResponseJson(error_response);
        }
    };
    let insert_stmt = r#"
        INSERT INTO trading_pools (id, creator_pubkey, name, strategy, token_bucket, total_amount_available_to_trade, trading_end_time, management_fee, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
        RETURNING id
    "#;
    let pool_id = uuid::Uuid::new_v4().to_string();
    let token_bucket_json = serde_json::to_value(&request.token_bucket).unwrap();
    let result = client.query_one(
        insert_stmt,
        &[&pool_id, &request.creator_pubkey, &request.pool_name, &request.strategy, &token_bucket_json, &request.total_amount_available_to_trade, &request.trading_end_time, &request.management_fee]
    ).await;
    if let Err(e) = result {
        tracing::error!("[start_trading] DB insert error: {}", e);
        let error_response = ApiResponse::<UnsignedTransactionResponse>::error("DB insert error".to_string());
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

    let agent_config = crate::agent::trading_agent::TradingAgentConfigBuilder::new()
        .with_token_pairs(token_pairs)
        .with_strategy_configs(vec![strategy_config])
        .with_portfolio_id(uuid::Uuid::parse_str(&pool_id).unwrap())
        .build();
    if let Ok(config) = agent_config {
        let icm_client = state.icm_client.clone();
        let db_pool = state.db.pool().clone();
        tokio::spawn(async move {
            let _ = crate::agent::trading_agent::TradingAgent::new(config, icm_client, db_pool).await;
        });
    } else {
        tracing::error!("[start_trading] Invalid agent config");
    }

    let tx_response = UnsignedTransactionResponse {
        transaction: pool_id.clone(),
        message: format!("Trading pool created and agent started: {}", pool_id),
    };
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
        platform_fee_bps: request.platform_fee_bps,
        route_plan: request.route_plan.clone(),
    };
    // You may need to parse/lookup these pubkeys as needed for your deployment
    let bucket_name = &request.bucket;
    let input_mint = Pubkey::from_str(&request.input_mint).unwrap();
    let output_mint = Pubkey::from_str(&request.output_mint).unwrap();

    // Fetch input_mint_program and output_mint_program from the database using the input/output mint addresses
    let pool = state.db.pool();
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[swap_tokens] DB connection error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error("DB connection error".to_string());
            return ResponseJson(error_response);
        }
    };
    // Query for mint program addresses
    let row = match client.query_opt(
        "SELECT input_mint_program, output_mint_program FROM token_mint_programs WHERE input_mint = $1 AND output_mint = $2",
        &[&request.input_mint, &request.output_mint],
    ).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("[swap_tokens] DB query error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error("DB query error".to_string());
            return ResponseJson(error_response);
        }
    };
    let (input_mint_program, output_mint_program) = match row {
        Some(row) => {
            let inp: String = row.try_get("input_mint_program").unwrap_or_default();
            let outp: String = row.try_get("output_mint_program").unwrap_or_default();
            (
                Pubkey::from_str(&inp).expect("Invalid input_mint_program from DB"),
                Pubkey::from_str(&outp).expect("Invalid output_mint_program from DB"),
            )
        },
        None => {
            tracing::error!("[swap_tokens] No mint program mapping found in DB");
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error("No mint program mapping found in DB".to_string());
            return ResponseJson(error_response);
        }
    };

    let jupiter_program = Pubkey::from_str(&std::env::var("JUPITER_PROGRAM_PUBKEY").expect("JUPITER_PROGRAM_PUBKEY env var required")).expect("Invalid JUPITER_PROGRAM_PUBKEY");
    let platform_fee_account = Pubkey::from_str(&std::env::var("PLATFORM_FEE_ACCOUNT").expect("PLATFORM_FEE_ACCOUNT env var required")).expect("Invalid PLATFORM_FEE_ACCOUNT");
    // For token_2022_program, use the constant from spl_token
    let token_2022_program = TOKEN_2022_PROGRAM_ID;

    match state.icm_client.agent_swap_tokens_transaction(
        instance_request,
        keypair,
        bucket_name,
        input_mint,
        output_mint,
        jupiter_program,
        token_2022_program,
        platform_fee_account,
        input_mint_program,
        output_mint_program,
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
        platform_fee_bps: request.platform_fee_bps,
        route_plan: request.route_plan.clone(),
    };
    // You may need to parse/lookup these pubkeys as needed for your deployment
    let bucket_name = &request.bucket;
    let input_mint = Pubkey::from_str(&request.input_mint).unwrap();
    let output_mint = Pubkey::from_str(&request.output_mint).unwrap();

    // Fetch input_mint_program and output_mint_program from the database using the input/output mint addresses
    let pool = state.db.pool();
    let client = match pool.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[agent_swap_tokens] DB connection error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error("DB connection error".to_string());
            return ResponseJson(error_response);
        }
    };
    // Query for mint program addresses
    let row = match client.query_opt(
        "SELECT input_mint_program, output_mint_program FROM token_mint_programs WHERE input_mint = $1 AND output_mint = $2",
        &[&request.input_mint, &request.output_mint],
    ).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("[agent_swap_tokens] DB query error: {}", e);
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error("DB query error".to_string());
            return ResponseJson(error_response);
        }
    };
    let (input_mint_program, output_mint_program) = match row {
        Some(row) => {
            let inp: String = row.try_get("input_mint_program").unwrap_or_default();
            let outp: String = row.try_get("output_mint_program").unwrap_or_default();
            (
                Pubkey::from_str(&inp).expect("Invalid input_mint_program from DB"),
                Pubkey::from_str(&outp).expect("Invalid output_mint_program from DB"),
            )
        },
        None => {
            tracing::error!("[agent_swap_tokens] No mint program mapping found in DB");
            let error_response = ApiResponse::<UnsignedTransactionResponse>::error("No mint program mapping found in DB".to_string());
            return ResponseJson(error_response);
        }
    };

    
    let jupiter_program = Pubkey::from_str(&std::env::var("JUPITER_PROGRAM_PUBKEY").expect("JUPITER_PROGRAM_PUBKEY env var required")).expect("Invalid JUPITER_PROGRAM_PUBKEY");
    let platform_fee_account = Pubkey::from_str(&std::env::var("PLATFORM_FEE_ACCOUNT").expect("PLATFORM_FEE_ACCOUNT env var required")).expect("Invalid PLATFORM_FEE_ACCOUNT");
    // For token_2022_program, use the constant from spl_token_2022
    let token_2022_program = TOKEN_2022_PROGRAM_ID;

    match state.icm_client.agent_swap_tokens_transaction(
        instance_request,
        keypair,
        bucket_name,
        input_mint,
        output_mint,
        jupiter_program,
        token_2022_program,
        platform_fee_account,
        input_mint_program,
        output_mint_program,
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

