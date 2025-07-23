//! Database Models
//!
//! Tokio-postgres compatible models for all database entities in the ICM system.
//! Includes trading agent data persistence for analytics and learning.

use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio_postgres::Row;
use uuid::Uuid;
use rust_decimal::Decimal;

/// Helper function to convert rust_decimal::Decimal to bigdecimal::BigDecimal
fn decimal_to_bigdecimal(decimal: Decimal) -> BigDecimal {
    BigDecimal::from_str(&decimal.to_string()).unwrap_or_default()
}

/// Trait for converting from tokio-postgres Row
pub trait FromRow {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> where Self: Sized;
}

// ============================================================================
// USER & AUTH MODELS
// ============================================================================

/// User account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub wallet_address: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow for User {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            password_hash: row.try_get("password_hash")?,
            wallet_address: row.try_get("wallet_address")?,
            is_active: row.try_get("is_active")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Portfolio/Bag containing multiple assets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
    pub is_active: bool,
    pub total_value_usd: BigDecimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow for Portfolio {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            is_public: row.try_get("is_public")?,
            is_active: row.try_get("is_active")?,
            total_value_usd: decimal_to_bigdecimal(row.try_get::<_, Decimal>("total_value_usd")?),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Asset allocation within a portfolio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioAsset {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub asset_symbol: String,
    pub asset_type: String, // "ETF", "STOCK", "CRYPTO"
    pub target_allocation_percent: BigDecimal,
    pub current_allocation_percent: BigDecimal,
    pub quantity: BigDecimal,
    pub average_cost_usd: BigDecimal,
    pub current_value_usd: BigDecimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow for PortfolioAsset {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            portfolio_id: row.try_get("portfolio_id")?,
            asset_symbol: row.try_get("asset_symbol")?,
            asset_type: row.try_get("asset_type")?,
            target_allocation_percent: decimal_to_bigdecimal(row.try_get::<_, Decimal>("target_allocation_percent")?),
            current_allocation_percent: decimal_to_bigdecimal(row.try_get::<_, Decimal>("current_allocation_percent")?),
            quantity: decimal_to_bigdecimal(row.try_get::<_, Decimal>("quantity")?),
            average_cost_usd: decimal_to_bigdecimal(row.try_get::<_, Decimal>("average_cost_usd")?),
            current_value_usd: decimal_to_bigdecimal(row.try_get::<_, Decimal>("current_value_usd")?),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Swap/Exchange transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Swap {
    pub id: Uuid,
    pub user_id: Uuid,
    pub portfolio_id: Option<Uuid>,
    pub from_currency: String,
    pub to_currency: String,
    pub from_amount: BigDecimal,
    pub to_amount: BigDecimal,
    pub exchange_rate: BigDecimal,
    pub fee_amount: BigDecimal,
    pub fee_currency: String,
    pub status: String, // "PENDING", "COMPLETED", "FAILED"
    pub transaction_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl FromRow for Swap {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            portfolio_id: row.try_get("portfolio_id")?,
            from_currency: row.try_get("from_currency")?,
            to_currency: row.try_get("to_currency")?,
            from_amount: decimal_to_bigdecimal(row.try_get::<_, Decimal>("from_amount")?),
            to_amount: decimal_to_bigdecimal(row.try_get::<_, Decimal>("to_amount")?),
            exchange_rate: decimal_to_bigdecimal(row.try_get::<_, Decimal>("exchange_rate")?),
            fee_amount: decimal_to_bigdecimal(row.try_get::<_, Decimal>("fee_amount")?),
            fee_currency: row.try_get("fee_currency")?,
            status: row.try_get("status")?,
            transaction_hash: row.try_get("transaction_hash")?,
            created_at: row.try_get("created_at")?,
            completed_at: row.try_get("completed_at")?,
        })
    }
}

/// Trading agents for automated portfolio management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub name: String,
    pub strategy_type: String,     // "DCA", "REBALANCE", "TREND_FOLLOW"
    pub config: serde_json::Value, // JSON configuration for the strategy
    pub is_active: bool,
    pub last_execution: Option<DateTime<Utc>>,
    pub next_execution: Option<DateTime<Utc>>,
    pub performance_fee_percent: BigDecimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FromRow for Agent {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            portfolio_id: row.try_get("portfolio_id")?,
            name: row.try_get("name")?,
            strategy_type: row.try_get("strategy_type")?,
            config: row.try_get("config")?,
            is_active: row.try_get("is_active")?,
            last_execution: row.try_get("last_execution")?,
            next_execution: row.try_get("next_execution")?,
            performance_fee_percent: decimal_to_bigdecimal(row.try_get::<_, Decimal>("performance_fee_percent")?),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// Agent execution logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentExecution {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub execution_type: String, // "REBALANCE", "BUY", "SELL"
    pub status: String,         // "SUCCESS", "FAILED", "PARTIAL"
    pub details: serde_json::Value,
    pub profit_loss_usd: Option<BigDecimal>,
    pub executed_at: DateTime<Utc>,
}

impl FromRow for AgentExecution {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            agent_id: row.try_get("agent_id")?,
            execution_type: row.try_get("execution_type")?,
            status: row.try_get("status")?,
            details: row.try_get("details")?,
            profit_loss_usd: row.try_get::<_, Option<Decimal>>("profit_loss_usd")?.map(decimal_to_bigdecimal),
            executed_at: row.try_get("executed_at")?,
        })
    }
}

/// Social feed items for portfolio performance and updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub id: Uuid,
    pub user_id: Uuid,
    pub portfolio_id: Option<Uuid>,
    pub item_type: String, // "PERFORMANCE", "TRADE", "MILESTONE"
    pub title: String,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub is_public: bool,
    pub created_at: DateTime<Utc>,
}

impl FromRow for FeedItem {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            portfolio_id: row.try_get("portfolio_id")?,
            item_type: row.try_get("item_type")?,
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            metadata: row.try_get("metadata")?,
            is_public: row.try_get("is_public")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

/// Performance tracking for portfolios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub id: Uuid,
    pub portfolio_id: Uuid,
    pub snapshot_date: DateTime<Utc>,
    pub total_value_usd: BigDecimal,
    pub daily_return_percent: Option<BigDecimal>,
    pub total_return_percent: BigDecimal,
    pub sharpe_ratio: Option<BigDecimal>,
    pub max_drawdown_percent: Option<BigDecimal>,
    pub created_at: DateTime<Utc>,
}

impl FromRow for PerformanceSnapshot {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            portfolio_id: row.try_get("portfolio_id")?,
            snapshot_date: row.try_get("snapshot_date")?,
            total_value_usd: decimal_to_bigdecimal(row.try_get::<_, Decimal>("total_value_usd")?),
            daily_return_percent: row.try_get::<_, Option<Decimal>>("daily_return_percent")?.map(decimal_to_bigdecimal),
            total_return_percent: decimal_to_bigdecimal(row.try_get::<_, Decimal>("total_return_percent")?),
            sharpe_ratio: row.try_get::<_, Option<Decimal>>("sharpe_ratio")?.map(decimal_to_bigdecimal),
            max_drawdown_percent: row.try_get::<_, Option<Decimal>>("max_drawdown_percent")?.map(decimal_to_bigdecimal),
            created_at: row.try_get("created_at")?,
        })
    }
}

/// Asset price data for tracking and calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetPrice {
    pub id: Uuid,
    pub symbol: String,
    pub price_usd: BigDecimal,
    pub volume_24h: Option<BigDecimal>,
    pub market_cap: Option<BigDecimal>,
    pub source: String, // "BINANCE", "COINBASE", "YAHOO_FINANCE"
    pub timestamp: DateTime<Utc>,
}

impl FromRow for AssetPrice {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            symbol: row.try_get("symbol")?,
            price_usd: decimal_to_bigdecimal(row.try_get::<_, Decimal>("price_usd")?),
            volume_24h: row.try_get::<_, Option<Decimal>>("volume_24h")?.map(decimal_to_bigdecimal),
            market_cap: row.try_get::<_, Option<Decimal>>("market_cap")?.map(decimal_to_bigdecimal),
            source: row.try_get("source")?,
            timestamp: row.try_get("timestamp")?,
        })
    }
}

/// Create new user request
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub wallet_address: Option<String>,
}

/// Create portfolio request
#[derive(Debug, Deserialize)]
pub struct CreatePortfolioRequest {
    pub name: String,
    pub description: Option<String>,
    pub is_public: bool,
}

/// Create swap request
#[derive(Debug, Deserialize)]
pub struct CreateSwapRequest {
    pub portfolio_id: Option<Uuid>,
    pub from_currency: String,
    pub to_currency: String,
    pub from_amount: BigDecimal,
}

// ============================================================================
// TRADING AGENT MODELS
// ============================================================================

/// Trading session record for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub strategy_type: String,
    pub config: serde_json::Value,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status: String, // Active, Paused, Stopped, Completed
    pub total_trades: i32,
    pub successful_trades: i32,
    pub total_pnl: BigDecimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Individual trade execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeExecution {
    pub id: Uuid,
    pub session_id: Uuid,
    pub trade_id: String,
    pub strategy_type: String,
    pub input_token: String,
    pub output_token: String,
    pub input_amount: BigDecimal,
    pub output_amount: Option<BigDecimal>,
    pub expected_output: BigDecimal,
    pub slippage: Option<BigDecimal>,
    pub gas_fee: Option<BigDecimal>,
    pub transaction_signature: Option<String>,
    pub status: String, // Pending, Success, Failed, Cancelled
    pub error_message: Option<String>,
    pub execution_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Market data snapshots for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshot {
    pub id: Uuid,
    pub symbol: String,
    pub price: BigDecimal,
    pub volume_24h: Option<BigDecimal>,
    pub price_change_24h: Option<BigDecimal>,
    pub liquidity: Option<BigDecimal>,
    pub timestamp: DateTime<Utc>,
    pub source: String, // jupiter, pyth, etc
}

/// AI decision logs for learning and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIDecision {
    pub id: Uuid,
    pub session_id: Uuid,
    pub decision_type: String, // trade, risk_assessment, strategy_adjustment
    pub input_data: serde_json::Value,
    pub output_decision: serde_json::Value,
    pub confidence_score: Option<BigDecimal>,
    pub execution_result: Option<String>,
    pub feedback_score: Option<BigDecimal>,
    pub timestamp: DateTime<Utc>,
}

/// Strategy performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    pub id: Uuid,
    pub strategy_type: String,
    pub time_period: String, // 1h, 4h, 1d, 1w, etc
    pub total_trades: i32,
    pub successful_trades: i32,
    pub total_pnl: BigDecimal,
    pub max_drawdown: BigDecimal,
    pub sharpe_ratio: Option<BigDecimal>,
    pub win_rate: BigDecimal,
    pub avg_trade_duration: Option<i64>, // seconds
    pub calculated_at: DateTime<Utc>,
}

// ============================================================================
// DATABASE IMPLEMENTATIONS
// ============================================================================

impl FromRow for TradingSession {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            strategy_type: row.try_get("strategy_type")?,
            config: row.try_get("config")?,
            start_time: row.try_get("start_time")?,
            end_time: row.try_get("end_time")?,
            status: row.try_get("status")?,
            total_trades: row.try_get("total_trades")?,
            successful_trades: row.try_get("successful_trades")?,
            total_pnl: decimal_to_bigdecimal(row.try_get::<_, Decimal>("total_pnl")?),
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl FromRow for TradeExecution {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            trade_id: row.try_get("trade_id")?,
            strategy_type: row.try_get("strategy_type")?,
            input_token: row.try_get("input_token")?,
            output_token: row.try_get("output_token")?,
            input_amount: decimal_to_bigdecimal(row.try_get::<_, Decimal>("input_amount")?),
            output_amount: row.try_get::<_, Option<Decimal>>("output_amount")?.map(decimal_to_bigdecimal),
            expected_output: decimal_to_bigdecimal(row.try_get::<_, Decimal>("expected_output")?),
            slippage: row.try_get::<_, Option<Decimal>>("slippage")?.map(decimal_to_bigdecimal),
            gas_fee: row.try_get::<_, Option<Decimal>>("gas_fee")?.map(decimal_to_bigdecimal),
            transaction_signature: row.try_get("transaction_signature")?,
            status: row.try_get("status")?,
            error_message: row.try_get("error_message")?,
            execution_time: row.try_get("execution_time")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl FromRow for AIDecision {
    fn from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            session_id: row.try_get("session_id")?,
            decision_type: row.try_get("decision_type")?,
            input_data: row.try_get("input_data")?,
            output_decision: row.try_get("output_decision")?,
            confidence_score: row.try_get::<_, Option<Decimal>>("confidence_score")?.map(decimal_to_bigdecimal),
            execution_result: row.try_get("execution_result")?,
            feedback_score: row.try_get::<_, Option<Decimal>>("feedback_score")?.map(decimal_to_bigdecimal),
            timestamp: row.try_get("timestamp")?,
        })
    }
}
