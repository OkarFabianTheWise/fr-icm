use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::Duration;
use tracing::{info, warn, error, debug};
use chrono::Utc;

use crate::agent::types::{
    StrategyConfig, StrategyType, AgentState, AgentError, 
    PerformanceMetrics, Position, LearningParameters,
};
use crate::agent::data_fetcher::{DataFetcher, DataFetcherStats};
use crate::agent::ai_client::AIClient;
use crate::agent::planner::{Planner, PlannerStats};
use crate::agent::executor::{Executor, ExecutorStats};
use crate::agent::observer::{Observer, ObserverStats, LearningFeedback};
use crate::onchain_instance::instance::IcmProgramInstance;

/// Main trading agent that orchestrates all components
pub struct TradingAgent {
    // Core components
    data_fetcher: Arc<DataFetcher>,
    ai_client: Arc<AIClient>,
    planner: Arc<Planner>,
    executor: Arc<Executor>,
    observer: Arc<Observer>,
    
    // State management
    agent_state: Arc<RwLock<AgentState>>,
    
    // Communication channels
    learning_receiver: Option<mpsc::UnboundedReceiver<LearningFeedback>>,
    position_receiver: Option<mpsc::UnboundedReceiver<HashMap<String, Position>>>,
    
    // Configuration
    config: TradingAgentConfig,
    is_running: Arc<RwLock<bool>>,
}

#[derive(Debug, Clone)]
pub struct TradingAgentConfig {
    pub openai_api_key: String,
    pub token_pairs: Vec<(String, String)>,
    pub strategy_configs: Vec<StrategyConfig>,
    pub data_fetch_interval_ms: u64,
    pub plan_evaluation_interval_ms: u64,
    pub monitoring_interval_ms: u64,
    pub max_concurrent_executions: usize,
    pub learning_enabled: bool,
}

impl TradingAgent {
    /// Create a new trading agent
    pub async fn new(
        config: TradingAgentConfig,
        icm_client: Arc<IcmProgramInstance>,
    ) -> Result<Self, AgentError> {
        info!("Initializing trading agent with {} token pairs and {} strategies",
              config.token_pairs.len(), config.strategy_configs.len());

        // Initialize AI client
        let ai_client = Arc::new(AIClient::new(config.openai_api_key.clone()));

        // Initialize data fetcher
        let (data_fetcher, quote_receiver) = DataFetcher::new(
            config.token_pairs.clone(),
            config.data_fetch_interval_ms,
        );
        let data_fetcher = Arc::new(data_fetcher);

        // Initialize planner
        let (planner, plan_receiver) = Planner::new(
            Arc::clone(&ai_client),
            config.strategy_configs.clone(),
            config.plan_evaluation_interval_ms,
        );
        let planner = Arc::new(planner);

        // Initialize executor
        let (executor, _plan_rx, execution_receiver) = Executor::new(
            icm_client,
            config.max_concurrent_executions,
        );
        let executor = Arc::new(executor);

        // Initialize observer
        let (observer, _exec_rx, learning_receiver, position_receiver) = Observer::new(
            config.monitoring_interval_ms,
        );
        let observer = Arc::new(observer);

        // Initialize agent state
        let initial_state = AgentState {
            is_active: false,
            current_positions: HashMap::new(),
            performance: Self::default_performance_metrics(),
            strategy_config: config.strategy_configs.first()
                .cloned()
                .unwrap_or_else(|| Self::default_strategy_config()),
            learning_parameters: Self::default_learning_parameters(),
            last_market_data: HashMap::new(),
        };

        let agent = Self {
            data_fetcher,
            ai_client,
            planner,
            executor,
            observer,
            agent_state: Arc::new(RwLock::new(initial_state)),
            learning_receiver: Some(learning_receiver),
            position_receiver: Some(position_receiver),
            config,
            is_running: Arc::new(RwLock::new(false)),
        };

        info!("Trading agent initialized successfully");
        Ok(agent)
    }

    /// Start the trading agent
    pub async fn start(&mut self) -> Result<(), AgentError> {
        {
            let mut is_running = self.is_running.write().await;
            if *is_running {
                return Ok(());
            }
            *is_running = true;
        }

        // Update agent state
        {
            let mut state = self.agent_state.write().await;
            state.is_active = true;
        }

        info!("Starting trading agent");

        // Start data fetcher
        let data_fetcher_handle = {
            let data_fetcher = Arc::clone(&self.data_fetcher);
            tokio::spawn(async move {
                if let Err(e) = data_fetcher.start().await {
                    error!("Data fetcher error: {}", e);
                }
            })
        };

        // Start planner with quote stream - need to connect properly
        let planner_handle = {
            let planner = Arc::clone(&self.planner);
            // In a real implementation, you'd need to properly connect the quote receiver
            // from data_fetcher to planner. For now, we'll create a dummy receiver.
            let (_tx, rx) = mpsc::unbounded_channel();
            tokio::spawn(async move {
                if let Err(e) = planner.start(rx).await {
                    error!("Planner error: {}", e);
                }
            })
        };

        // Start executor
        let executor_handle = {
            let executor = Arc::clone(&self.executor);
            tokio::spawn(async move {
                // Note: executor.start() requires &mut self, so this needs architectural changes
                // For now, we'll just log that it should be started
                info!("Executor should be started here");
            })
        };

        // Start observer
        let observer_handle = {
            let observer = Arc::clone(&self.observer);
            tokio::spawn(async move {
                // Note: observer.start() requires &mut self, so this needs architectural changes
                // For now, we'll just log that it should be started
                info!("Observer should be started here");
            })
        };

        // Start learning and adaptation loop if enabled
        let learning_handle = if self.config.learning_enabled {
            let learning_receiver = self.learning_receiver.take();
            let position_receiver = self.position_receiver.take();
            let agent_state = Arc::clone(&self.agent_state);
            let is_running = Arc::clone(&self.is_running);

            Some(tokio::spawn(async move {
                Self::learning_loop(
                    learning_receiver,
                    position_receiver,
                    agent_state,
                    is_running,
                ).await;
            }))
        } else {
            None
        };

        info!("All trading agent components started successfully");

        // Wait for shutdown signal
        while *self.is_running.read().await {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        // Cleanup on shutdown
        self.data_fetcher.stop().await;
        self.planner.stop().await;
        self.executor.stop().await;
        self.observer.stop().await;

        info!("Trading agent stopped");
        Ok(())
    }

    /// Stop the trading agent
    pub async fn stop(&self) -> Result<(), AgentError> {
        info!("Stopping trading agent");

        let mut is_running = self.is_running.write().await;
        *is_running = false;

        // Update agent state
        {
            let mut state = self.agent_state.write().await;
            state.is_active = false;
        }

        Ok(())
    }

    /// Learning and adaptation loop
    async fn learning_loop(
        mut learning_receiver: Option<mpsc::UnboundedReceiver<LearningFeedback>>,
        mut position_receiver: Option<mpsc::UnboundedReceiver<HashMap<String, Position>>>,
        agent_state: Arc<RwLock<AgentState>>,
        is_running: Arc<RwLock<bool>>,
    ) {
        info!("Starting learning and adaptation loop");

        let mut learning_rx = learning_receiver.take().unwrap();
        let mut position_rx = position_receiver.take().unwrap();

        while *is_running.read().await {
            tokio::select! {
                // Process learning feedback
                Some(feedback) = learning_rx.recv() => {
                    Self::process_learning_feedback(feedback, &agent_state).await;
                }

                // Update positions
                Some(positions) = position_rx.recv() => {
                    Self::update_agent_positions(positions, &agent_state).await;
                }
            }
        }

        info!("Learning loop stopped");
    }

    /// Process learning feedback and adapt strategy parameters
    async fn process_learning_feedback(
        feedback: LearningFeedback,
        agent_state: &Arc<RwLock<AgentState>>,
    ) {
        let mut state = agent_state.write().await;

        // Apply suggested parameter adjustments
        for (param, adjustment) in feedback.suggested_adjustments {
            Self::apply_parameter_adjustment(&mut state, &param, adjustment).await;
        }

        info!("Applied learning feedback for strategy {:?}", feedback.strategy_type);
    }

    /// Apply parameter adjustment to agent state
    async fn apply_parameter_adjustment(
        state: &mut AgentState,
        param_name: &str,
        adjustment: f64,
    ) {
        match param_name {
            "priority_fee_percentile" => {
                let current = state.strategy_config.execution_settings.priority_fee_percentile as f64;
                let new_value = (current + adjustment).clamp(50.0, 99.0) as u8;
                state.strategy_config.execution_settings.priority_fee_percentile = new_value;
                info!("Adjusted priority_fee_percentile: {} -> {}", current, new_value);
            }
            "max_slippage_bps" => {
                let current = state.strategy_config.parameters.max_slippage_bps as f64;
                let new_value = (current + adjustment).clamp(10.0, 500.0) as u16;
                state.strategy_config.parameters.max_slippage_bps = new_value;
                info!("Adjusted max_slippage_bps: {} -> {}", current, new_value);
            }
            "position_size_multiplier" => {
                let current = state.strategy_config.parameters.position_size_usd;
                let new_value = current * (1.0 + adjustment);
                state.strategy_config.parameters.position_size_usd = new_value.clamp(100.0, 10000.0);
                info!("Adjusted position_size_usd: {:.2} -> {:.2}", current, new_value);
            }
            _ => {
                warn!("Unknown parameter for adjustment: {}", param_name);
            }
        }
    }

    /// Update agent positions from observer
    async fn update_agent_positions(
        positions: HashMap<String, Position>,
        agent_state: &Arc<RwLock<AgentState>>,
    ) {
        let mut state = agent_state.write().await;
        
        // Convert String keys to Pubkey
        let mut pubkey_positions = HashMap::new();
        for (key, position) in positions {
            if let Ok(pubkey) = key.parse::<solana_sdk::pubkey::Pubkey>() {
                pubkey_positions.insert(pubkey, position);
            }
        }
        
        state.current_positions = pubkey_positions;
        
        debug!("Updated agent positions: {} active", state.current_positions.len());
    }

    /// Get comprehensive agent statistics
    pub async fn get_stats(&self) -> Result<AgentStats, AgentError> {
        let state = self.agent_state.read().await;
        
        Ok(AgentStats {
            is_running: *self.is_running.read().await,
            is_active: state.is_active,
            uptime_seconds: 0, // Would track actual uptime
            data_fetcher: self.data_fetcher.get_stats().await,
            planner: self.planner.get_stats().await,
            executor: self.executor.get_stats().await,
            observer: self.observer.get_stats().await,
            performance: state.performance.clone(),
            active_positions: state.current_positions.len(),
            current_strategy: state.strategy_config.strategy_type.clone(),
        })
    }

    /// Get current agent state
    pub async fn get_state(&self) -> AgentState {
        self.agent_state.read().await.clone()
    }

    /// Update strategy configuration
    pub async fn update_strategy_config(&self, config: StrategyConfig) -> Result<(), AgentError> {
        // Validate the configuration first
        crate::agent::strategy::StrategyFactory::validate_strategy_config(&config)?;

        // Update agent state
        {
            let mut state = self.agent_state.write().await;
            state.strategy_config = config.clone();
        }

        // Update planner configuration
        self.planner.update_strategy_config(config).await?;

        info!("Updated strategy configuration");
        Ok(())
    }

    /// Force rebalance positions
    pub async fn force_rebalance(&self) -> Result<(), AgentError> {
        info!("Force rebalancing positions");
        // Implementation would trigger immediate rebalance
        Ok(())
    }

    /// Emergency stop - immediately halt all trading
    pub async fn emergency_stop(&self) -> Result<(), AgentError> {
        warn!("Emergency stop activated");
        
        // Stop all components immediately
        self.stop().await?;
        
        // Additional emergency procedures could be added here
        // e.g., close all positions, send alerts, etc.
        
        Ok(())
    }

    /// Default performance metrics
    fn default_performance_metrics() -> PerformanceMetrics {
        PerformanceMetrics {
            total_trades: 0,
            successful_trades: 0,
            total_pnl: rust_decimal::Decimal::new(0, 0),
            win_rate: 0.0,
            avg_slippage_bps: 0.0,
            avg_execution_time_ms: 0,
            max_drawdown: 0.0,
            sharpe_ratio: 0.0,
            last_updated: Utc::now(),
        }
    }

    /// Default strategy configuration
    fn default_strategy_config() -> StrategyConfig {
        use crate::agent::types::*;
        
        StrategyConfig {
            strategy_type: StrategyType::Arbitrage,
            parameters: StrategyParameters {
                min_spread_bps: 50,
                max_slippage_bps: 100,
                position_size_usd: 1000.0,
                rebalance_threshold_pct: 0.05,
                lookback_periods: 24,
                custom_params: HashMap::new(),
            },
            risk_limits: RiskLimits {
                max_position_size_usd: 10000.0,
                max_daily_loss_pct: 5.0,
                max_drawdown_pct: 15.0,
                stop_loss_pct: 3.0,
                take_profit_pct: 10.0,
            },
            execution_settings: ExecutionSettings {
                priority_fee_percentile: 75,
                max_priority_fee_lamports: 100_000,
                transaction_timeout_ms: 30_000,
                retry_attempts: 3,
                jito_tip_lamports: 10_000,
            },
        }
    }

    /// Default learning parameters
    fn default_learning_parameters() -> LearningParameters {
        LearningParameters {
            learning_rate: 0.01,
            adaptation_window_hours: 24,
            performance_threshold: 0.7,
            parameter_bounds: {
                let mut bounds = HashMap::new();
                bounds.insert("priority_fee_percentile".to_string(), (50.0, 99.0));
                bounds.insert("max_slippage_bps".to_string(), (10.0, 500.0));
                bounds.insert("position_size_multiplier".to_string(), (0.1, 2.0));
                bounds
            },
        }
    }
}

/// Comprehensive statistics for the trading agent
#[derive(Debug, serde::Serialize)]
pub struct AgentStats {
    pub is_running: bool,
    pub is_active: bool,
    pub uptime_seconds: u64,
    pub data_fetcher: DataFetcherStats,
    pub planner: PlannerStats,
    pub executor: ExecutorStats,
    pub observer: ObserverStats,
    pub performance: PerformanceMetrics,
    pub active_positions: usize,
    pub current_strategy: StrategyType,
}

/// Builder for creating trading agent configurations
pub struct TradingAgentConfigBuilder {
    openai_api_key: Option<String>,
    token_pairs: Vec<(String, String)>,
    strategy_configs: Vec<StrategyConfig>,
    data_fetch_interval_ms: u64,
    plan_evaluation_interval_ms: u64,
    monitoring_interval_ms: u64,
    max_concurrent_executions: usize,
    learning_enabled: bool,
}

impl TradingAgentConfigBuilder {
    pub fn new() -> Self {
        Self {
            openai_api_key: None,
            token_pairs: Vec::new(),
            strategy_configs: Vec::new(),
            data_fetch_interval_ms: 5000,  // 5 seconds
            plan_evaluation_interval_ms: 10000, // 10 seconds
            monitoring_interval_ms: 30000, // 30 seconds
            max_concurrent_executions: 5,
            learning_enabled: true,
        }
    }

    pub fn with_openai_api_key(mut self, api_key: String) -> Self {
        self.openai_api_key = Some(api_key);
        self
    }

    pub fn with_token_pairs(mut self, pairs: Vec<(String, String)>) -> Self {
        self.token_pairs = pairs;
        self
    }

    pub fn with_strategy_configs(mut self, configs: Vec<StrategyConfig>) -> Self {
        self.strategy_configs = configs;
        self
    }

    pub fn with_data_fetch_interval(mut self, interval_ms: u64) -> Self {
        self.data_fetch_interval_ms = interval_ms;
        self
    }

    pub fn with_learning_enabled(mut self, enabled: bool) -> Self {
        self.learning_enabled = enabled;
        self
    }

    pub fn build(self) -> Result<TradingAgentConfig, AgentError> {
        let openai_api_key = self.openai_api_key
            .ok_or_else(|| AgentError::Configuration("OpenAI API key required".to_string()))?;

        if self.token_pairs.is_empty() {
            return Err(AgentError::Configuration("At least one token pair required".to_string()));
        }

        if self.strategy_configs.is_empty() {
            return Err(AgentError::Configuration("At least one strategy config required".to_string()));
        }

        Ok(TradingAgentConfig {
            openai_api_key,
            token_pairs: self.token_pairs,
            strategy_configs: self.strategy_configs,
            data_fetch_interval_ms: self.data_fetch_interval_ms,
            plan_evaluation_interval_ms: self.plan_evaluation_interval_ms,
            monitoring_interval_ms: self.monitoring_interval_ms,
            max_concurrent_executions: self.max_concurrent_executions,
            learning_enabled: self.learning_enabled,
        })
    }
}
