# Technical Architecture - How Components Connect

This document explains how all the ICM Server components technically connect and communicate with each other.

## 🏗️ System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        HTTP Server (Axum)                       │
│                     ┌─────────────────────┐                     │
│                     │   AppState (Shared) │                     │
│                     │  ┌─────────────────┐ │                     │
│                     │  │ ICM Program     │ │                     │
│                     │  │ Instance        │ │                     │
│                     │  └─────────────────┘ │                     │
│                     │  ┌─────────────────┐ │                     │
│                     │  │ Trading Agent   │ │                     │
│                     │  │ (Arc<RwLock>)   │ │                     │
│                     │  └─────────────────┘ │                     │
│                     └─────────────────────┘                     │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Trading Agent                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ Data Fetcher│  │   Planner   │  │  Observer   │              │
│  │             │  │             │  │             │              │
│  │ - Prices    │  │ - Strategies│  │ - Metrics   │              │
│  │ - Market    │  │ - AI Plans  │  │ - Alerts    │              │
│  │ - WebSocket │  │ - Risk Mgmt │  │ - History   │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
│         │                 │                 ▲                  │
│         ▼                 ▼                 │                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   Executor                              │    │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │    │
│  │  │ Concurrent  │  │ Jupiter API │  │ ICM Program │     │    │
│  │  │ Execution   │  │ Integration │  │ Integration │     │    │
│  │  │ (Semaphore) │  │             │  │             │     │    │
│  │  └─────────────┘  └─────────────┘  └─────────────┘     │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                    External Systems                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ Jupiter DEX │  │ Solana RPC  │  │ OpenAI API  │              │
│  │ - Quotes    │  │ - Txns      │  │ - Strategy  │              │
│  │ - Routes    │  │ - Accounts  │  │ - Analysis  │              │
│  │ - Swaps     │  │ - Blocks    │  │ - Learning  │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘
```

## 🔄 Component Communication Patterns

### 1. **Message Passing (MPSC Channels)**

```rust
// Executor receives trading plans from Planner
let (plan_sender, plan_receiver) = mpsc::unbounded_channel::<TradingPlan>();

// Executor sends results to Observer
let (result_sender, result_receiver) = mpsc::unbounded_channel::<ExecutionResult>();

// Data Fetcher broadcasts market updates
let (market_sender, market_receiver) = broadcast::channel::<MarketUpdate>(1000);
```

**Flow:**

1. **Planner** → **Executor**: Trading plans via MPSC channel
2. **Executor** → **Observer**: Execution results via MPSC channel
3. **Data Fetcher** → **All Components**: Market data via broadcast channel

### 2. **Shared State (Arc<RwLock<T>>)**

```rust
pub struct TradingAgent {
    pub state: Arc<RwLock<AgentState>>,
    pub performance: Arc<RwLock<PerformanceMetrics>>,
    pub config: Arc<RwLock<TradingAgentConfig>>,
}

pub struct AppState {
    pub icm_client: Arc<IcmProgramInstance>,
    pub trading_agent: Arc<RwLock<Option<TradingAgent>>>,
}
```

**Access Pattern:**

- **Read Access**: Multiple components can read simultaneously
- **Write Access**: Exclusive write access with blocking
- **Thread Safety**: Arc provides shared ownership across threads

### 3. **Direct Function Calls**

```rust
impl Executor {
    // Direct call to Jupiter API
    async fn get_jupiter_swap_instructions(&self, plan: &TradingPlan) -> Result<Value, AgentError>

    // Direct call to ICM Program
    async fn execute_swap(&self, plan: &TradingPlan) -> Result<UnsignedTransactionResponse, AgentError>
}
```

## 📊 Data Flow Diagrams

### Trading Execution Flow

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ Data Fetcher│───▶│   Planner   │───▶│  Executor   │───▶│  Observer   │
│             │    │             │    │             │    │             │
│ Market Data │    │ Trading Plan│    │ Execute Txn │    │ Track Result│
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
      │                   │                   │                   │
      ▼                   ▼                   ▼                   ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ WebSocket   │    │ OpenAI API  │    │ Jupiter DEX │    │ Database    │
│ Feeds       │    │ Strategy    │    │ Solana RPC  │    │ Metrics     │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
```

### HTTP Request Flow

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ HTTP Client │───▶│ Axum Server │───▶│ Route Handler│───▶│Trading Agent│
│             │    │             │    │             │    │             │
│ POST /start │    │ AppState    │    │ start_agent │    │ Initialize  │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
      ▲                                                           │
      │                                                           ▼
┌─────────────┐                                          ┌─────────────┐
│ JSON        │◀─────────────────────────────────────────│ Agent Stats │
│ Response    │                                          │ & Status    │
└─────────────┘                                          └─────────────┘
```

## 🔧 Component Initialization Sequence

### 1. **Server Startup** (`src/main.rs` → `src/server.rs`)

```rust
#[tokio::main]
async fn main() {
    // 1. Initialize logging
    tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).init();

    // 2. Create ICM Program instance
    let icm_instance = IcmProgramInstance::new(Cluster::Devnet, keypair)?;

    // 3. Create application state
    let app_state = AppState {
        icm_client: Arc::new(icm_instance),
        trading_agent: Arc::new(RwLock::new(None)),
    };

    // 4. Setup routes with shared state
    let app = Router::new()
        .route("/api/v1/agent/start", post(start_agent))
        .with_state(app_state);

    // 5. Start server
    axum::serve(listener, app).await;
}
```

### 2. **Agent Initialization** (When `/api/v1/agent/start` is called)

```rust
pub async fn start_agent(State(state): State<AppState>, Json(request): Json<StartAgentRequest>) {
    // 1. Build agent configuration
    let config = TradingAgentConfigBuilder::new()
        .openai_api_key(request.openai_api_key)
        .token_pairs(request.token_pairs)
        .build();

    // 2. Create channel network
    let (plan_tx, plan_rx) = mpsc::unbounded_channel();
    let (result_tx, result_rx) = mpsc::unbounded_channel();
    let (market_tx, market_rx) = broadcast::channel(1000);

    // 3. Initialize components
    let data_fetcher = DataFetcher::new(market_tx.clone(), config.clone());
    let planner = Planner::new(plan_tx, market_rx.resubscribe(), config.clone());
    let executor = Executor::new(Arc::clone(&state.icm_client), 5);
    let observer = Observer::new(result_rx, config.clone());

    // 4. Create trading agent
    let agent = TradingAgent::new(config, data_fetcher, planner, executor, observer);

    // 5. Store in shared state
    let mut agent_guard = state.trading_agent.write().await;
    *agent_guard = Some(agent);
}
```

### 3. **Component Startup Sequence**

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ Data Fetcher│───▶│   Planner   │───▶│  Executor   │───▶│  Observer   │
│   START     │    │   START     │    │   START     │    │   START     │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
      │                   │                   │                   │
      ▼                   ▼                   ▼                   ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ WebSocket   │    │ AI Strategy │    │ Semaphore   │    │ Metrics     │
│ Connection  │    │ Loop        │    │ Permits     │    │ Collection  │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
```

## 🔀 Inter-Component Communication Details

### 1. **Data Fetcher → Planner**

```rust
// Data Fetcher broadcasts market updates
#[derive(Clone, Debug)]
pub struct MarketUpdate {
    pub symbol: String,
    pub price: f64,
    pub volume_24h: f64,
    pub volatility: f64,
    pub timestamp: DateTime<Utc>,
}

// Planner receives and processes updates
impl Planner {
    async fn market_update_loop(&mut self) {
        while let Ok(update) = self.market_receiver.recv().await {
            self.process_market_update(update).await;
            if let Some(plan) = self.generate_trading_plan().await {
                self.plan_sender.send(plan).unwrap();
            }
        }
    }
}
```

### 2. **Planner → Executor**

```rust
// Planner sends trading plans
#[derive(Debug, Clone)]
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

// Executor receives and executes plans
impl ExecutorHandle {
    async fn execute_plan(&self, plan: TradingPlan) {
        let result = match self.execute_swap(&plan).await {
            Ok(tx_response) => ExecutionResult { /* success */ },
            Err(e) => ExecutionResult { /* failure */ },
        };

        self.result_sender.send(result).unwrap();
    }
}
```

### 3. **Executor → Observer**

```rust
// Executor sends execution results
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub plan_id: Uuid,
    pub success: bool,
    pub transaction_signature: Option<String>,
    pub execution_time_ms: u64,
    pub actual_slippage_bps: Option<u16>,
    pub error_message: Option<String>,
    pub gas_used: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

// Observer processes results and updates metrics
impl Observer {
    async fn result_processing_loop(&mut self) {
        while let Some(result) = self.result_receiver.recv().await {
            self.update_performance_metrics(&result).await;
            self.check_alert_conditions(&result).await;
            self.store_execution_history(result).await;
        }
    }
}
```

## 🛡️ Error Handling & Recovery

### 1. **Component Failure Isolation**

```rust
impl TradingAgent {
    async fn monitor_components(&self) {
        loop {
            // Check each component health
            if !self.data_fetcher.is_healthy().await {
                tracing::error!("Data Fetcher unhealthy, restarting...");
                self.restart_data_fetcher().await;
            }

            if !self.executor.is_healthy().await {
                tracing::error!("Executor unhealthy, stopping trading...");
                self.emergency_stop().await;
            }

            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
}
```

### 2. **Circuit Breaker Pattern**

```rust
pub struct CircuitBreaker {
    pub failure_count: AtomicU32,
    pub last_failure: AtomicU64,
    pub threshold: u32,
    pub timeout_ms: u64,
}

impl CircuitBreaker {
    pub fn should_allow_request(&self) -> bool {
        let failures = self.failure_count.load(Ordering::Relaxed);
        if failures < self.threshold {
            return true;
        }

        let now = Utc::now().timestamp_millis() as u64;
        let last_failure = self.last_failure.load(Ordering::Relaxed);

        now - last_failure > self.timeout_ms
    }
}
```

### 3. **Graceful Shutdown**

```rust
impl TradingAgent {
    pub async fn shutdown(&mut self) -> Result<(), AgentError> {
        tracing::info!("Initiating graceful shutdown...");

        // 1. Stop accepting new trades
        self.planner.stop().await?;

        // 2. Wait for active executions to complete
        self.executor.wait_for_completion().await?;

        // 3. Save final metrics
        self.observer.flush_metrics().await?;

        // 4. Close connections
        self.data_fetcher.disconnect().await?;

        tracing::info!("Shutdown complete");
        Ok(())
    }
}
```

## 📈 Performance Optimizations

### 1. **Async/Await Concurrency**

- All components run concurrently using Tokio
- Non-blocking I/O for network operations
- Efficient task scheduling and resource utilization

### 2. **Memory Management**

- Arc<T> for shared ownership without copying
- RwLock for reader-writer synchronization
- Channel-based message passing to avoid locks

### 3. **Caching Strategy**

```rust
pub struct DataCache {
    quotes: DashMap<String, CachedQuote>,
    prices: DashMap<String, CachedPrice>,
    market_data: DashMap<String, CachedMarketData>,
}

impl DataCache {
    pub fn get_quote(&self, pair: &str) -> Option<JupiterQuote> {
        self.quotes.get(pair)
            .filter(|entry| !entry.is_expired())
            .map(|entry| entry.data.clone())
    }
}
```

This technical architecture ensures robust, scalable, and maintainable communication between all ICM Server components while providing excellent performance and reliability.
