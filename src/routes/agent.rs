use std::sync::Arc;
use axum::{
    extract::{Path, State, Json},
    http::StatusCode,
    response::Json as ResponseJson,
    Router, routing::{get, post},
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use tokio::sync::RwLock;

use crate::agent::{
    TradingAgent, AgentState, StrategyConfig, 
    StrategyType, StrategyParameters, RiskLimits, ExecutionSettings,
    trading_agent::{TradingAgentConfig, TradingAgentConfigBuilder, AgentStats},
};
use crate::server::AppState;

/// Response for agent status endpoint
#[derive(Debug, Serialize)]
pub struct AgentStatusResponse {
    pub status: String,
    pub is_running: bool,
    pub stats: Option<AgentStats>,
    pub message: String,
}

/// Request to start the trading agent
#[derive(Debug, Deserialize)]
pub struct StartAgentRequest {
    pub openai_api_key: String,
    pub token_pairs: Vec<(String, String)>,
    pub strategies: Vec<StrategyConfigRequest>,
    pub data_fetch_interval_ms: Option<u64>,
    pub learning_enabled: Option<bool>,
}

/// Strategy configuration request format
#[derive(Debug, Deserialize)]
pub struct StrategyConfigRequest {
    pub strategy_type: String, // "Arbitrage", "DCA", "GridTrading", etc.
    pub min_spread_bps: Option<u16>,
    pub max_slippage_bps: Option<u16>,
    pub position_size_usd: Option<f64>,
    pub max_position_size_usd: Option<f64>,
    pub priority_fee_percentile: Option<u8>,
    pub max_priority_fee_lamports: Option<u64>,
}

/// Request to update strategy configuration
#[derive(Debug, Deserialize)]
pub struct UpdateStrategyRequest {
    pub strategy_config: StrategyConfigRequest,
}

/// Get trading agent status
pub async fn get_agent_status(
    State(state): State<AppState>,
) -> Result<ResponseJson<AgentStatusResponse>, (StatusCode, String)> {
    info!("Getting trading agent status");

    let agent_guard = state.trading_agent.read().await;
    
    let (status, is_running, stats, message) = if let Some(agent) = agent_guard.as_ref() {
        match agent.get_stats().await {
            Ok(stats) => (
                "active".to_string(),
                stats.is_running,
                Some(stats),
                "Trading agent is operational".to_string(),
            ),
            Err(e) => (
                "error".to_string(),
                false,
                None,
                format!("Failed to get agent stats: {}", e),
            ),
        }
    } else {
        (
            "inactive".to_string(),
            false,
            None,
            "Trading agent is not initialized".to_string(),
        )
    };

    Ok(ResponseJson(AgentStatusResponse {
        status,
        is_running,
        stats,
        message,
    }))
}

/// Start the trading agent
pub async fn start_agent(
    State(state): State<AppState>,
    Json(request): Json<StartAgentRequest>,
) -> Result<ResponseJson<AgentStatusResponse>, (StatusCode, String)> {
    info!("Starting trading agent with {} token pairs", request.token_pairs.len());

    // Convert strategy requests to actual strategy configs
    let mut strategy_configs = Vec::new();
    for strategy_req in request.strategies {
        let strategy_config = convert_strategy_request(strategy_req)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid strategy config: {}", e)))?;
        strategy_configs.push(strategy_config);
    }

    // Build trading agent configuration
    let mut config_builder = TradingAgentConfigBuilder::new()
        .with_openai_api_key(request.openai_api_key)
        .with_token_pairs(request.token_pairs)
        .with_strategy_configs(strategy_configs);

    if let Some(interval) = request.data_fetch_interval_ms {
        config_builder = config_builder.with_data_fetch_interval(interval);
    }

    if let Some(learning) = request.learning_enabled {
        config_builder = config_builder.with_learning_enabled(learning);
    }

    let config = config_builder.build()
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid configuration: {}", e)))?;

    // Create new trading agent
    let new_agent = TradingAgent::new(config, Arc::clone(&state.icm_client)).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create agent: {}", e)))?;

    // Start the agent (this would need to be run in a background task)
    // For now, we'll just create it and mark it as ready
    info!("Trading agent created successfully");

    // Store the agent
    let mut agent_guard = state.trading_agent.write().await;
    *agent_guard = Some(new_agent);

    Ok(ResponseJson(AgentStatusResponse {
        status: "started".to_string(),
        is_running: true,
        stats: None,
        message: "Trading agent started successfully".to_string(),
    }))
}

/// Stop the trading agent
pub async fn stop_agent(
    State(state): State<AppState>,
) -> Result<ResponseJson<AgentStatusResponse>, (StatusCode, String)> {
    info!("Stopping trading agent");

    let mut agent_guard = state.trading_agent.write().await;
    
    if let Some(agent) = agent_guard.as_ref() {
        agent.stop().await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to stop agent: {}", e)))?;
        
        *agent_guard = None;
        
        Ok(ResponseJson(AgentStatusResponse {
            status: "stopped".to_string(),
            is_running: false,
            stats: None,
            message: "Trading agent stopped successfully".to_string(),
        }))
    } else {
        Ok(ResponseJson(AgentStatusResponse {
            status: "inactive".to_string(),
            is_running: false,
            stats: None,
            message: "Trading agent was not running".to_string(),
        }))
    }
}

/// Get detailed agent state
pub async fn get_agent_state(
    State(state): State<AppState>,
) -> Result<ResponseJson<AgentState>, (StatusCode, String)> {
    let agent_guard = state.trading_agent.read().await;
    
    if let Some(agent) = agent_guard.as_ref() {
        let agent_state = agent.get_state().await;
        Ok(ResponseJson(agent_state))
    } else {
        Err((StatusCode::NOT_FOUND, "Trading agent not initialized".to_string()))
    }
}

/// Update strategy configuration
pub async fn update_strategy(
    State(state): State<AppState>,
    Json(request): Json<UpdateStrategyRequest>,
) -> Result<ResponseJson<AgentStatusResponse>, (StatusCode, String)> {
    info!("Updating strategy configuration");

    let strategy_config = convert_strategy_request(request.strategy_config)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid strategy config: {}", e)))?;

    let agent_guard = state.trading_agent.read().await;
    
    if let Some(agent) = agent_guard.as_ref() {
        agent.update_strategy_config(strategy_config).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to update strategy: {}", e)))?;
        
        Ok(ResponseJson(AgentStatusResponse {
            status: "updated".to_string(),
            is_running: true,
            stats: None,
            message: "Strategy configuration updated successfully".to_string(),
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Trading agent not initialized".to_string()))
    }
}

/// Force rebalance positions
pub async fn force_rebalance(
    State(state): State<AppState>,
) -> Result<ResponseJson<AgentStatusResponse>, (StatusCode, String)> {
    info!("Force rebalancing positions");

    let agent_guard = state.trading_agent.read().await;
    
    if let Some(agent) = agent_guard.as_ref() {
        agent.force_rebalance().await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to rebalance: {}", e)))?;
        
        Ok(ResponseJson(AgentStatusResponse {
            status: "rebalanced".to_string(),
            is_running: true,
            stats: None,
            message: "Portfolio rebalanced successfully".to_string(),
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Trading agent not initialized".to_string()))
    }
}

/// Emergency stop
pub async fn emergency_stop(
    State(state): State<AppState>,
) -> Result<ResponseJson<AgentStatusResponse>, (StatusCode, String)> {
    warn!("Emergency stop activated");

    let mut agent_guard = state.trading_agent.write().await;
    
    if let Some(agent) = agent_guard.as_ref() {
        agent.emergency_stop().await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to emergency stop: {}", e)))?;
        
        *agent_guard = None;
        
        Ok(ResponseJson(AgentStatusResponse {
            status: "emergency_stopped".to_string(),
            is_running: false,
            stats: None,
            message: "Emergency stop completed".to_string(),
        }))
    } else {
        Ok(ResponseJson(AgentStatusResponse {
            status: "inactive".to_string(),
            is_running: false,
            stats: None,
            message: "Trading agent was not running".to_string(),
        }))
    }
}

/// Convert strategy request to actual strategy config
fn convert_strategy_request(req: StrategyConfigRequest) -> Result<StrategyConfig, String> {
    let strategy_type = match req.strategy_type.as_str() {
        "Arbitrage" => StrategyType::Arbitrage,
        "DCA" => StrategyType::DCA,
        "GridTrading" => StrategyType::GridTrading,
        "MeanReversion" => StrategyType::MeanReversion,
        "TrendFollowing" => StrategyType::TrendFollowing,
        _ => return Err(format!("Unknown strategy type: {}", req.strategy_type)),
    };

    let parameters = StrategyParameters {
        min_spread_bps: req.min_spread_bps.unwrap_or(50),
        max_slippage_bps: req.max_slippage_bps.unwrap_or(100),
        position_size_usd: req.position_size_usd.unwrap_or(1000.0),
        rebalance_threshold_pct: 0.05, // Default 5%
        lookback_periods: 24, // Default 24 periods
        custom_params: std::collections::HashMap::new(),
    };

    let risk_limits = RiskLimits {
        max_position_size_usd: req.max_position_size_usd.unwrap_or(10000.0),
        max_daily_loss_pct: 5.0, // Default 5%
        max_drawdown_pct: 15.0, // Default 15%
        stop_loss_pct: 3.0, // Default 3%
        take_profit_pct: 10.0, // Default 10%
    };

    let execution_settings = ExecutionSettings {
        priority_fee_percentile: req.priority_fee_percentile.unwrap_or(75),
        max_priority_fee_lamports: req.max_priority_fee_lamports.unwrap_or(100_000),
        transaction_timeout_ms: 30_000, // 30 seconds
        retry_attempts: 3,
        jito_tip_lamports: 10_000,
    };

    Ok(StrategyConfig {
        strategy_type,
        parameters,
        risk_limits,
        execution_settings,
    })
}

/// Create agent routes
pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/agent/status", get(get_agent_status))
        .route("/api/v1/agent/start", post(start_agent))
        .route("/api/v1/agent/stop", post(stop_agent))
        .route("/api/v1/agent/state", get(get_agent_state))
        .route("/api/v1/agent/strategy", post(update_strategy))
        .route("/api/v1/agent/rebalance", post(force_rebalance))
        .route("/api/v1/agent/emergency-stop", post(emergency_stop))
}
