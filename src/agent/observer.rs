use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::time::interval;
use tracing::{info, warn, debug, error};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Deserialize;
use crate::agent::types::{
    Position, PerformanceMetrics, AgentError, StrategyType,
};
use crate::agent::executor::ExecutionResult;
use crate::database::models::{Portfolio, PortfolioAsset};
// Jupiter price API endpoint
const JUPITER_PRICE_API: &str = "https://price.jup.ag/v4/price";

#[derive(Debug, Deserialize)]
struct JupiterPriceResponse {
    data: HashMap<String, JupiterTokenPrice>,
}

#[derive(Debug, Deserialize)]
struct JupiterTokenPrice {
    price: f64,
}

/// Observer monitors execution results and provides feedback for learning
#[derive(Debug)]
pub struct Observer {
    execution_receiver: Option<mpsc::UnboundedReceiver<ExecutionResult>>,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
    active_positions: Arc<DashMap<String, Position>>,
    execution_history: Arc<RwLock<Vec<ExecutionResult>>>,
    learning_feedback: mpsc::UnboundedSender<LearningFeedback>,
    position_updates: mpsc::UnboundedSender<HashMap<String, Position>>,
    is_active: Arc<RwLock<bool>>,
    monitoring_interval: Duration,
    db_pool: deadpool_postgres::Pool,
    data_fetcher: Arc<crate::agent::data_fetcher::DataFetcher>,
    portfolio_id: uuid::Uuid,
}



#[derive(Debug, Clone)]
pub struct LearningFeedback {
    pub strategy_type: StrategyType,
    pub execution_result: ExecutionResult,
    pub position_change: Option<()>,
    pub performance_impact: PerformanceImpact,
    pub suggested_adjustments: HashMap<String, f64>,
}



#[derive(Debug, Clone)]
pub struct PerformanceImpact {
    pub pnl_impact: f64,
    pub win_rate_impact: f64,
    pub risk_score_change: f64,
    pub execution_quality: ExecutionQuality,
}

#[derive(Debug, Clone)]
pub enum ExecutionQuality {
    Excellent, // Better than expected
    Good,      // As expected
    Fair,      // Slightly worse than expected
    Poor,      // Significantly worse than expected
}

impl Observer {


    /// Fetch the list of tokens from the database for the configured portfolio
    async fn fetch_monitored_tokens(&self) -> Vec<String> {
        match PortfolioAsset::fetch_token_mints_by_portfolio(&self.db_pool, self.portfolio_id).await {
            Ok(tokens) => tokens,
            Err(e) => {
                error!("Failed to fetch token mints from DB: {}", e);
                vec![]
            }
        }
    }

    /// Fetch prices for a list of tokens using the DataFetcher cache
    async fn fetch_token_prices(&self, tokens: &[String]) -> HashMap<String, f64> {
        let mut prices = HashMap::new();
        for token in tokens {
            if let Some(price) = self.data_fetcher.get_cached_price(token) {
                prices.insert(token.clone(), price);
            } else {
                warn!("No cached price for token {}", token);
            }
        }
        prices
    }

    pub async fn start_with_receiver(&mut self, execution_receiver: mpsc::UnboundedReceiver<ExecutionResult>) -> Result<(), AgentError> {
        self.execution_receiver = Some(execution_receiver);
        // You can add your monitoring loop logic here
        Ok(())
    }
    pub fn new(
        monitoring_interval_ms: u64,
        db_pool: deadpool_postgres::Pool,
        data_fetcher: Arc<crate::agent::data_fetcher::DataFetcher>,
        portfolio_id: uuid::Uuid,
    ) -> (Self, mpsc::UnboundedReceiver<ExecutionResult>, mpsc::UnboundedReceiver<LearningFeedback>, mpsc::UnboundedReceiver<HashMap<String, Position>>) {
        let (exec_sender, exec_receiver) = mpsc::unbounded_channel();
        let (feedback_sender, feedback_receiver) = mpsc::unbounded_channel();
        let (position_sender, position_receiver) = mpsc::unbounded_channel();

        let observer = Self {
            execution_receiver: None, // We'll set this after creation
            performance_metrics: Arc::new(RwLock::new(Self::default_performance_metrics())),
            active_positions: Arc::new(DashMap::new()),
            execution_history: Arc::new(RwLock::new(Vec::new())),
            learning_feedback: feedback_sender,
            position_updates: position_sender,
            is_active: Arc::new(RwLock::new(false)),
            monitoring_interval: Duration::from_millis(monitoring_interval_ms),
            db_pool,
            data_fetcher,
            portfolio_id,
        };

        (observer, exec_receiver, feedback_receiver, position_receiver)
    }

    /// Set the execution receiver (called after creation)
    pub fn set_execution_receiver(&mut self, receiver: mpsc::UnboundedReceiver<ExecutionResult>) {
        self.execution_receiver = Some(receiver);
    }

    /// Start the observer monitoring loop
    pub async fn start(&mut self) -> Result<(), AgentError> {
        {
            let mut is_active = self.is_active.write().await;
            if *is_active {
                return Ok(());
            }
            *is_active = true;
        }

        let mut execution_receiver = self.execution_receiver.take()
            .ok_or_else(|| AgentError::Configuration("Observer already started".to_string()))?;

        info!("Starting observer");

        let mut monitoring_timer = interval(self.monitoring_interval);

        while *self.is_active.read().await {
            tokio::select! {
                // Process execution results
                Some(result) = execution_receiver.recv() => {
                    self.process_execution_result(result).await;
                }

                // Periodic monitoring and cleanup
                _ = monitoring_timer.tick() => {
                    self.perform_periodic_monitoring().await;
                }
            }
        }

        info!("Observer stopped");
        Ok(())
    }

    /// Stop the observer
    pub async fn stop(&self) {
        let mut is_active = self.is_active.write().await;
        *is_active = false;
        info!("Observer stop signal sent");
    }

    /// Process a single execution result
    async fn process_execution_result(&self, result: ExecutionResult) {
        info!("Processing execution result for plan {}: success={}", 
              result.plan_id, result.success);

        // Store execution result
        {
            let mut history = self.execution_history.write().await;
            history.push(result.clone());
            
            // Keep only recent results to prevent memory bloat
            if history.len() > 10000 {
                history.drain(0..1000); // Remove oldest 1000 results
            }
        }

        // Update performance metrics
        self.update_performance_metrics(&result).await;

        // Update position tracking
        self.update_position_tracking(&result).await;

        // Generate learning feedback
        self.generate_learning_feedback(result).await;

        // Send position updates to planner
        let positions: HashMap<String, Position> = self.active_positions.iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        
        if let Err(e) = self.position_updates.send(positions) {
            warn!("Failed to send position updates: {}", e);
        }
    }

    /// Update overall performance metrics
    async fn update_performance_metrics(&self, result: &ExecutionResult) {
        let mut metrics = self.performance_metrics.write().await;
        
        metrics.total_trades += 1;
        if result.success {
            metrics.successful_trades += 1;
        }

        // Update win rate
        metrics.win_rate = metrics.successful_trades as f64 / metrics.total_trades as f64;

        // Update average execution time
        if metrics.total_trades == 1 {
            metrics.avg_execution_time_ms = result.execution_time_ms;
        } else {
            metrics.avg_execution_time_ms = (
                (metrics.avg_execution_time_ms * (metrics.total_trades - 1)) + result.execution_time_ms
            ) / metrics.total_trades;
        }

        // Update average slippage if available
        if let Some(slippage) = result.actual_slippage_bps {
            let current_total = metrics.avg_slippage_bps * (metrics.total_trades - 1) as f64;
            metrics.avg_slippage_bps = (current_total + slippage as f64) / metrics.total_trades as f64;
        }

        metrics.last_updated = Utc::now();

        debug!("Updated performance metrics: total_trades={}, win_rate={:.2}%, avg_time={}ms",
               metrics.total_trades, metrics.win_rate * 100.0, metrics.avg_execution_time_ms);
    }

    /// Update position tracking based on execution results
    async fn update_position_tracking(&self, result: &ExecutionResult) {
        // In a real implementation, you would:
        // 1. Query on-chain state to get actual position changes
        // 2. Calculate PnL based on entry/exit prices
        // 3. Update position sizes and values
        
        // For now, we'll simulate position updates
        let position_key = format!("{}_{}", result.plan_id, result.timestamp.timestamp());
        
        if result.success {
            // Simulate a successful trade creating or modifying a position
            let simulated_position = Position {
                bucket_pubkey: solana_sdk::pubkey::Pubkey::new_unique(),
                token_mint: solana_sdk::pubkey::Pubkey::new_unique(),
                amount: 1000000, // 1 token
                entry_price: 100.0,
                current_price: 100.0,
                unrealized_pnl: 0.0,
                opened_at: result.timestamp,
            };

            self.active_positions.insert(position_key, simulated_position);
            debug!("Updated position tracking for successful execution");
        } else {
            debug!("No position update for failed execution");
        }
    }

    /// Generate learning feedback based on execution results
    async fn generate_learning_feedback(&self, result: ExecutionResult) {
        // Calculate execution quality
        let execution_quality = self.assess_execution_quality(&result).await;

        // Generate suggested parameter adjustments
        let suggested_adjustments = self.generate_parameter_adjustments(&result, &execution_quality).await;

        // Calculate performance impact
        let performance_impact = self.calculate_performance_impact(&result).await;

        let feedback = LearningFeedback {
            strategy_type: StrategyType::Arbitrage, // Would be determined from plan
            execution_result: result,
            position_change: None, // Would be calculated from actual position changes
            performance_impact,
            suggested_adjustments,
        };

        if let Err(e) = self.learning_feedback.send(feedback) {
            warn!("Failed to send learning feedback: {}", e);
        }
    }

    /// Assess the quality of execution
    async fn assess_execution_quality(&self, result: &ExecutionResult) -> ExecutionQuality {
        if !result.success {
            return ExecutionQuality::Poor;
        }

        // Assess based on execution time and slippage
        let quality_score = if result.execution_time_ms < 2000 {
            1.0 // Fast execution
        } else if result.execution_time_ms < 5000 {
            0.7 // Moderate execution time
        } else {
            0.3 // Slow execution
        };

        let slippage_score = if let Some(slippage) = result.actual_slippage_bps {
            if slippage < 50 {
                1.0 // Low slippage
            } else if slippage < 100 {
                0.7 // Moderate slippage
            } else {
                0.3 // High slippage
            }
        } else {
            0.5 // Unknown slippage
        };

        let combined_score = (quality_score + slippage_score) / 2.0;

        match combined_score {
            s if s >= 0.8 => ExecutionQuality::Excellent,
            s if s >= 0.6 => ExecutionQuality::Good,
            s if s >= 0.4 => ExecutionQuality::Fair,
            _ => ExecutionQuality::Poor,
        }
    }

    /// Generate parameter adjustment suggestions
    async fn generate_parameter_adjustments(
        &self,
        result: &ExecutionResult,
        quality: &ExecutionQuality,
    ) -> HashMap<String, f64> {
        let mut adjustments = HashMap::new();

        match quality {
            ExecutionQuality::Poor => {
                // Increase priority fees for faster execution
                adjustments.insert("priority_fee_percentile".to_string(), 5.0);
                // Increase slippage tolerance
                adjustments.insert("max_slippage_bps".to_string(), 10.0);
                // Reduce position size to lower risk
                adjustments.insert("position_size_multiplier".to_string(), -0.1);
            },
            ExecutionQuality::Fair => {
                // Moderate adjustments
                adjustments.insert("priority_fee_percentile".to_string(), 2.0);
                adjustments.insert("max_slippage_bps".to_string(), 5.0);
            },
            ExecutionQuality::Good => {
                // Small optimizations
                adjustments.insert("position_size_multiplier".to_string(), 0.05);
            },
            ExecutionQuality::Excellent => {
                // Try to optimize further
                adjustments.insert("priority_fee_percentile".to_string(), -1.0);
                adjustments.insert("position_size_multiplier".to_string(), 0.1);
            },
        }

        adjustments
    }

    /// Calculate performance impact of the execution
    async fn calculate_performance_impact(&self, result: &ExecutionResult) -> PerformanceImpact {
        let metrics = self.performance_metrics.read().await;
        
        // Calculate PnL impact (simplified)
        let pnl_impact = if result.success {
            // Assume positive impact for successful trades
            100.0 // $100 profit (simplified)
        } else {
            // Assume gas cost for failed trades
            -10.0 // $10 loss from gas fees
        };

        // Calculate win rate impact
        let old_win_rate = if metrics.total_trades <= 1 {
            0.0
        } else {
            (metrics.successful_trades - if result.success { 1 } else { 0 }) as f64 / (metrics.total_trades - 1) as f64
        };
        let win_rate_impact = metrics.win_rate - old_win_rate;

        // Calculate risk score change (simplified)
        let risk_score_change = if result.success { -0.01 } else { 0.05 };

        let execution_quality = self.assess_execution_quality(result).await;

        PerformanceImpact {
            pnl_impact,
            win_rate_impact,
            risk_score_change,
            execution_quality,
        }
    }

    /// Perform periodic monitoring tasks
    async fn perform_periodic_monitoring(&self) {
        debug!("Performing periodic monitoring");

        // 1. Fetch tokens from DB (simulate)
        let tokens = self.fetch_monitored_tokens().await;
        debug!("Monitoring tokens: {:?}", tokens);

        // 2. Fetch prices from Jupiter
        let prices = self.fetch_token_prices(&tokens).await;
        debug!("Fetched prices: {:?}", prices);

        // 3. Analyze with AI (stub: just log for now)
        // TODO: Replace with real AI analysis
        if !prices.is_empty() {
            info!("[AI] Analyzing token prices: {:?}", prices);
            // Example: If SOL price > 100, execute a trade (stub)
            if let Some(sol_price) = prices.get("So11111111111111111111111111111111111111112") {
                if *sol_price > 100.0 {
                    info!("[AI] SOL price > 100, would execute trade");
                    // TODO: Call execution logic here
                }
            }
        }

        // 4. Update position values based on current market prices
        self.update_position_values().await;

        // 5. Calculate and update performance metrics
        self.recalculate_performance_metrics().await;

        // 6. Clean up old data
        self.cleanup_old_data().await;

        // 7. Generate periodic reports
        self.generate_periodic_reports().await;
    }

    /// Update position values with current market prices
    async fn update_position_values(&self) {
        // In a real implementation, you would:
        // 1. Fetch current prices for all tokens in positions
        // 2. Update current_price and unrealized_pnl for each position
        // 3. Calculate total portfolio value

        for mut position in self.active_positions.iter_mut() {
            // Simulate price updates
            let price_change = (rand::random::<f64>() - 0.5) * 0.02; // ±1% random change
            position.current_price *= 1.0 + price_change;
            
            // Calculate unrealized PnL
            let price_change_pct = (position.current_price / position.entry_price) - 1.0;
            position.unrealized_pnl = position.amount as f64 * position.entry_price * price_change_pct;
        }
    }

    /// Recalculate comprehensive performance metrics
    async fn recalculate_performance_metrics(&self) {
        // Calculate total PnL from all positions
        let total_unrealized_pnl: f64 = self.active_positions.iter()
            .map(|pos| pos.unrealized_pnl)
            .sum();

        // Update Sharpe ratio calculation
        // In a real implementation, you would calculate this based on historical returns

        let mut metrics = self.performance_metrics.write().await;
        // Update total PnL to include unrealized gains
        let unrealized_decimal = rust_decimal::Decimal::from_f64_retain(total_unrealized_pnl)
            .unwrap_or_else(|| rust_decimal::Decimal::new(0, 0));
        
        // In real implementation, you'd have separate realized vs unrealized PnL tracking
        metrics.total_pnl = unrealized_decimal;

        debug!("Recalculated performance metrics: unrealized PnL = ${:.2}", total_unrealized_pnl);
    }

    /// Clean up old data to prevent memory bloat
    async fn cleanup_old_data(&self) {
        let cutoff_time = Utc::now() - chrono::Duration::days(7);

        // Clean up old execution history
        {
            let mut history = self.execution_history.write().await;
            history.retain(|result| result.timestamp > cutoff_time);
        }

        // Close old positions that might be stale
        self.active_positions.retain(|_key, position| {
            position.opened_at > cutoff_time
        });
    }

    /// Generate periodic performance reports
    async fn generate_periodic_reports(&self) {
        let metrics = self.performance_metrics.read().await;
        let position_count = self.active_positions.len();

        info!("Performance Report - Trades: {}, Win Rate: {:.1}%, Positions: {}, Avg Execution: {}ms",
              metrics.total_trades,
              metrics.win_rate * 100.0,
              position_count,
              metrics.avg_execution_time_ms);
    }

    /// Get current performance metrics
    pub async fn get_performance_metrics(&self) -> PerformanceMetrics {
        self.performance_metrics.read().await.clone()
    }

    /// Get current positions
    pub async fn get_positions(&self) -> HashMap<String, Position> {
        self.active_positions.iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Get observer statistics
    pub async fn get_stats(&self) -> ObserverStats {
        let metrics = self.performance_metrics.read().await;
        let history_size = self.execution_history.read().await.len();

        ObserverStats {
            is_active: *self.is_active.read().await,
            total_executions_monitored: metrics.total_trades,
            active_positions: self.active_positions.len(),
            execution_history_size: history_size,
            last_update: metrics.last_updated,
        }
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
}

#[derive(Debug, serde::Serialize)]
pub struct ObserverStats {
    pub is_active: bool,
    pub total_executions_monitored: u64,
    pub active_positions: usize,
    pub execution_history_size: usize,
    pub last_update: DateTime<Utc>,
}
