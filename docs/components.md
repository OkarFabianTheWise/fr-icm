# ICM Server Components Documentation

This document provides detailed information about each component in the ICM trading system.

## 🤖 AI Trading Agent (`src/agent/trading_agent.rs`)

### Overview

The central orchestrator that coordinates all trading activities using AI-powered decision making.

### Key Responsibilities

- **Strategy Management**: Manages multiple trading strategies simultaneously
- **Risk Assessment**: Evaluates market conditions and adjusts risk parameters
- **Performance Tracking**: Monitors and learns from trading outcomes
- **Component Coordination**: Orchestrates data fetcher, planner, executor, and observer

### Configuration

```rust
pub struct TradingAgentConfig {
    pub openai_api_key: String,
    pub max_concurrent_trades: usize,
    pub risk_threshold: f64,
    pub learning_enabled: bool,
    pub data_fetch_interval_ms: u64,
}
```

### States

- **Inactive**: Agent not initialized
- **Active**: Agent configured but not trading
- **Running**: Agent actively trading
- **Error**: Agent encountered an error

### API Integration

- Start agent: `POST /api/v1/agent/start`
- Stop agent: `POST /api/v1/agent/stop`
- Get status: `GET /api/v1/agent/status`

---

## ⚡ Executor (`src/agent/executor.rs`)

### Overview

**Your implementation!** Handles the actual execution of trading plans by building and submitting blockchain transactions.

### Key Features

- **Concurrent Execution**: Handles multiple trades simultaneously using semaphores
- **Jupiter Integration**: Connects to Jupiter DEX for optimal swap routing
- **Error Handling**: Robust error handling with retry mechanisms
- **Metrics Tracking**: Real-time performance metrics and execution statistics

### Core Functionality

```rust
impl Executor {
    // Executes trading plans concurrently
    pub async fn execute_plan(&self, plan: TradingPlan) -> ExecutionResult

    // Gets fresh swap instructions from Jupiter
    async fn get_jupiter_swap_instructions(&self, plan: &TradingPlan) -> Result<Value, AgentError>

    // Builds and submits transaction through ICM program
    async fn execute_swap(&self, plan: &TradingPlan) -> Result<UnsignedTransactionResponse, AgentError>
}
```

### Execution Flow

1. **Receive Plan**: Get trading plan from planner
2. **Acquire Permit**: Use semaphore for concurrency control
3. **Validate Plan**: Check expiration and parameters
4. **Get Route**: Fetch optimal route from Jupiter
5. **Build Transaction**: Create ICM program transaction
6. **Execute**: Submit to blockchain
7. **Track Results**: Update metrics and notify observer

### Performance Metrics

- Total executions
- Success rate
- Average execution time
- Gas usage statistics
- Error tracking

---

## 📊 Data Fetcher (`src/agent/data_fetcher.rs`)

### Overview

Collects real-time market data from multiple sources including price feeds, DEX data, and market indicators.

### Data Sources

- **Jupiter API**: Token prices and routing information
- **Solana RPC**: On-chain data and transaction status
- **WebSocket Feeds**: Real-time price updates
- **Historical Data**: Price history for trend analysis

### Caching Strategy

```rust
pub struct DataCache {
    pub quotes: HashMap<String, JupiterQuote>,
    pub prices: HashMap<String, TokenPrice>,
    pub last_update: DateTime<Utc>,
    pub ttl_seconds: u64,
}
```

### Key Functions

- **Price Monitoring**: Continuous price feed updates
- **Quote Aggregation**: Collects quotes from multiple DEXs
- **Market Conditions**: Volatility, volume, and liquidity analysis
- **Data Validation**: Ensures data quality and consistency

### Configuration

- Fetch interval: 1-30 seconds (configurable)
- Cache TTL: 30 seconds default
- Token pairs: Configurable list
- WebSocket reconnection: Automatic

---

## 🧠 Planner (`src/agent/planner.rs`)

### Overview

The strategic brain that analyzes market conditions and creates executable trading plans using AI insights.

### Strategy Types

```rust
pub enum StrategyType {
    Arbitrage,      // Price differences across DEXs
    DCA,           // Dollar Cost Averaging
    GridTrading,   // Range-bound trading
    Momentum,      // Trend following
}
```

### Planning Process

1. **Market Analysis**: Evaluate current market conditions
2. **Opportunity Detection**: Identify profitable trading opportunities
3. **Risk Assessment**: Calculate position sizes and risk parameters
4. **Plan Generation**: Create detailed execution plans
5. **AI Enhancement**: Use OpenAI for strategy optimization

### Risk Management

- **Position Sizing**: Dynamic position size calculation
- **Stop Loss**: Automatic loss protection
- **Take Profit**: Profit target management
- **Drawdown Limits**: Maximum loss thresholds

### Plan Structure

```rust
pub struct TradingPlan {
    pub id: Uuid,
    pub strategy_type: StrategyType,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub input_amount: u64,
    pub min_output_amount: u64,
    pub max_slippage_bps: u16,
    pub expires_at: DateTime<Utc>,
    pub route_plan: Vec<u8>,
    pub priority_fee: u64,
    pub bucket_pubkey: Pubkey,
}
```

---

## 👀 Observer (`src/agent/observer.rs`)

### Overview

Monitors all trading activities, tracks performance, and provides real-time analytics and alerting.

### Monitoring Capabilities

- **Trade Execution**: Real-time trade monitoring
- **Performance Metrics**: P&L, win rate, Sharpe ratio
- **Risk Monitoring**: Drawdown tracking, exposure limits
- **Alerting**: Configurable alerts and notifications

### Performance Tracking

```rust
pub struct PerformanceMetrics {
    pub total_trades: u64,
    pub successful_trades: u64,
    pub total_pnl: Decimal,
    pub win_rate: f64,
    pub avg_slippage_bps: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
}
```

### Alerting System

- **Trade Alerts**: Execution success/failure notifications
- **Risk Alerts**: Drawdown or exposure limit breaches
- **Performance Alerts**: Significant performance changes
- **System Alerts**: Component health and connectivity issues

### Data Storage

- In-memory for real-time access
- Database persistence for historical analysis
- Configurable retention periods
- Export capabilities for external analysis

---

## 🌐 HTTP Server (`src/server.rs`)

### Overview

Provides RESTful API endpoints for system control, monitoring, and integration.

### Route Organization

```
/ping                    # Health check
/api/v1/agent/*         # Agent control endpoints
/api/v1/bucket/*        # ICM program endpoints
```

### Features

- **Async Processing**: Built on Axum and Tokio
- **State Management**: Shared application state
- **Error Handling**: Consistent error responses
- **Logging**: Structured logging with tracing

### Middleware

- **CORS**: Cross-origin request support
- **Rate Limiting**: Request throttling (production)
- **Authentication**: JWT-based auth (future)
- **Request Logging**: Comprehensive request tracking

---

## 💾 Database Integration (`src/database/`)

### Overview

PostgreSQL integration for persistent data storage and retrieval.

### Schema Design

```sql
-- Trading history
CREATE TABLE trades (
    id UUID PRIMARY KEY,
    strategy_type VARCHAR(50),
    input_token VARCHAR(44),
    output_token VARCHAR(44),
    input_amount BIGINT,
    output_amount BIGINT,
    executed_at TIMESTAMP,
    success BOOLEAN
);

-- Performance metrics
CREATE TABLE performance_snapshots (
    id UUID PRIMARY KEY,
    total_pnl DECIMAL,
    win_rate DECIMAL,
    max_drawdown DECIMAL,
    created_at TIMESTAMP
);
```

### Connection Management

- **Connection Pooling**: Efficient connection reuse
- **Migration System**: Schema version management
- **Health Checks**: Database connectivity monitoring
- **Backup Strategy**: Automated backup procedures

---

## ⛓️ Solana Integration (`src/onchain_instance/`)

### Overview

Handles all blockchain interactions including transaction building, signing, and submission.

### ICM Program Interface

```rust
pub struct IcmProgramInstance {
    pub client: Client,
    pub payer: Keypair,
    pub program_id: Pubkey,
    pub cluster: Cluster,
}

impl IcmProgramInstance {
    pub async fn swap_tokens_transaction(
        &self,
        request: SwapTokensRequest,
    ) -> Result<UnsignedTransactionResponse, Box<dyn std::error::Error>>;
}
```

### Transaction Lifecycle

1. **Build Instruction**: Create program instruction
2. **Add Accounts**: Specify required accounts
3. **Calculate Fee**: Determine transaction cost
4. **Sign Transaction**: Apply wallet signature
5. **Submit**: Send to Solana network
6. **Confirm**: Wait for confirmation
7. **Monitor**: Track transaction status

### Error Handling

- **RPC Errors**: Network connectivity issues
- **Program Errors**: Smart contract failures
- **Account Errors**: Insufficient funds or permissions
- **Timeout Handling**: Transaction timeout management

---

## 🔧 Configuration Management (`src/config.rs`)

### Overview

Centralized configuration management with environment variable support.

### Configuration Structure

```rust
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub solana: SolanaConfig,
    pub trading: TradingConfig,
    pub ai: AiConfig,
}
```

### Environment Variables

```bash
# Server Configuration
SERVER_HOST=127.0.0.1
SERVER_PORT=3000

# Database Configuration
DATABASE_URL=postgresql://user:pass@localhost:5432/icm_db
DATABASE_MAX_CONNECTIONS=10

# Solana Configuration
SOLANA_RPC_URL=https://api.devnet.solana.com
SOLANA_WS_URL=wss://api.devnet.solana.com

# Trading Configuration
MAX_CONCURRENT_TRADES=5
RISK_THRESHOLD=0.8
QUOTE_CACHE_TTL=30

# AI Configuration
OPENAI_API_KEY=your_openai_api_key_here
```

### Validation

- **Required Fields**: Ensures all critical config is present
- **Format Validation**: Validates URLs, keys, and numeric ranges
- **Default Values**: Provides sensible defaults where appropriate
- **Environment Override**: Allows runtime configuration changes

---

## 🔐 Authentication (`src/auth/`)

### Overview

JWT-based authentication system for API security (future implementation).

### Components

- **JWT Generation**: Token creation and validation
- **Middleware**: Request authentication
- **User Management**: User registration and login
- **Role-Based Access**: Different permission levels

This completes the component documentation. Each component has clear responsibilities, well-defined interfaces, and comprehensive error handling.
