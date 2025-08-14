use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, Duration};
use tracing::{info, warn, error, debug};
use futures::StreamExt;

use crate::agent::types::{
    QuoteData, TradingPlan, StrategyConfig, StrategyType, AgentError,
    MarketConditions, Position, AIAnalysisRequest, ExecutionContext,
};
use crate::agent::strategy::{Strategy, StrategyFactory};
use crate::agent::ai_client::AIClient;

/// The planner evaluates market data and generates trading plans
pub struct Planner {
    strategies: HashMap<StrategyType, Box<dyn Strategy>>,
    ai_client: AIClient,
    plan_queue: mpsc::UnboundedSender<TradingPlan>,
    market_conditions: Arc<RwLock<MarketConditions>>,
    current_positions: Arc<RwLock<HashMap<String, Position>>>,
    strategy_configs: HashMap<StrategyType, StrategyConfig>,
    evaluation_interval: Duration,
    is_active: Arc<RwLock<bool>>,
}

impl Planner {
    pub fn new(
    ai_client: AIClient,
        strategy_configs: Vec<StrategyConfig>,
        evaluation_interval_ms: u64,
    ) -> (Self, mpsc::UnboundedReceiver<TradingPlan>) {
        let (plan_sender, plan_receiver) = mpsc::unbounded_channel();
        
        // Initialize strategies
        let mut strategies = HashMap::new();
        let mut configs_map = HashMap::new();
        
        for config in strategy_configs {
            let strategy = StrategyFactory::create_strategy(config.strategy_type.clone());
            strategies.insert(config.strategy_type.clone(), strategy);
            configs_map.insert(config.strategy_type.clone(), config);
        }

        let planner = Self {
            strategies,
            ai_client,
            plan_queue: plan_sender,
            market_conditions: Arc::new(RwLock::new(Self::default_market_conditions())),
            current_positions: Arc::new(RwLock::new(HashMap::new())),
            strategy_configs: configs_map,
            evaluation_interval: Duration::from_millis(evaluation_interval_ms),
            is_active: Arc::new(RwLock::new(false)),
        };

        (planner, plan_receiver)
    }

    /// Start the planning loop with market data stream
    pub async fn start(
        &self,
        mut quote_receiver: mpsc::UnboundedReceiver<QuoteData>,
    ) -> Result<(), AgentError> {
        {
            let mut is_active = self.is_active.write().await;
            if *is_active {
                return Ok(());
            }
            *is_active = true;
        }

        info!("Starting planner with {} strategies", self.strategies.len());

        let mut evaluation_timer = interval(self.evaluation_interval);
        let mut recent_quotes: Vec<QuoteData> = Vec::new();
        let max_recent_quotes = 100;

        while *self.is_active.read().await {
            tokio::select! {
                // Process incoming quotes
                Some(quote) = quote_receiver.recv() => {
                    debug!("Received quote for {}/{}", quote.input_mint, quote.output_mint);
                    
                    // Store recent quotes for analysis
                    recent_quotes.push(quote.clone());
                    if recent_quotes.len() > max_recent_quotes {
                        recent_quotes.remove(0);
                    }

                    // Update market conditions based on new data
                    if let Err(e) = self.update_market_conditions(&recent_quotes).await {
                        warn!("Failed to update market conditions: {}", e);
                    }

                    // Immediate evaluation for time-sensitive strategies (like arbitrage)
                    self.evaluate_time_sensitive_strategies(&quote).await;
                }

                // Periodic comprehensive evaluation
                _ = evaluation_timer.tick() => {
                    if !recent_quotes.is_empty() {
                        self.perform_comprehensive_evaluation(&recent_quotes).await;
                    }
                }
            }
        }

        info!("Planner stopped");
        Ok(())
    }

    /// Stop the planner
    pub async fn stop(&self) {
        let mut is_active = self.is_active.write().await;
        *is_active = false;
        info!("Planner stop signal sent");
    }

    /// Update current positions (called by executor/observer)
    pub async fn update_positions(&self, positions: HashMap<String, Position>) {
        let mut current_positions = self.current_positions.write().await;
        *current_positions = positions;
        info!("Updated {} positions", current_positions.len());
    }

    /// Add or update strategy configuration
    pub async fn update_strategy_config(&self, config: StrategyConfig) -> Result<(), AgentError> {
        // Validate configuration
        StrategyFactory::validate_strategy_config(&config)?;

        // This would require &mut self to actually update - in real implementation
        // you'd use Arc<RwLock<>> for strategy_configs too
        info!("Strategy config update requested for {:?}", config.strategy_type);
        Ok(())
    }

    /// Evaluate time-sensitive strategies (arbitrage, scalping)
    async fn evaluate_time_sensitive_strategies(&self, quote: &QuoteData) {
        let market_conditions = self.market_conditions.read().await;
        let positions = self.current_positions.read().await;

        // Only evaluate arbitrage and other time-sensitive strategies
        for (strategy_type, strategy) in &self.strategies {
            if matches!(strategy_type, StrategyType::Arbitrage) {
                if let Some(config) = self.strategy_configs.get(strategy_type) {
                    match strategy.evaluate(quote, &market_conditions, &positions, config).await {
                        Ok(Some(plan)) => {
                            info!("Time-sensitive plan generated: {:?} with confidence {}", 
                                  plan.strategy_type, plan.confidence_score);
                            
                            if let Err(e) = self.plan_queue.send(plan) {
                                error!("Failed to send plan to queue: {}", e);
                            }
                        }
                        Ok(None) => {
                            debug!("No time-sensitive opportunity for {:?}", strategy_type);
                        }
                        Err(e) => {
                            warn!("Error evaluating {:?} strategy: {}", strategy_type, e);
                        }
                    }
                }
            }
        }
    }

    /// Perform comprehensive evaluation with AI assistance
    async fn perform_comprehensive_evaluation(&self, recent_quotes: &[QuoteData]) {
        let _market_conditions = self.market_conditions.read().await;
        let positions = self.current_positions.read().await;

        // Get AI analysis for market conditions and strategy suggestions
        let ai_request = AIAnalysisRequest {
            market_data: recent_quotes.to_vec(),
            current_positions: positions.values().cloned().collect(),
            strategy_config: self.strategy_configs.values().next().cloned()
                .unwrap_or_else(|| self.default_strategy_config()),
            performance_history: self.default_performance_metrics(),
            question: "Analyze current market conditions and suggest optimal trading strategies".to_string(),
        };

    match self.ai_client.analyze_trading_opportunity(ai_request).await {
            Ok(ai_response) => {
                info!("AI analysis: {} (confidence: {})", 
                      ai_response.reasoning, ai_response.confidence);

                // Use AI insights to adjust strategy evaluation
                self.evaluate_strategies_with_ai_insights(recent_quotes, &ai_response).await;
            }
            Err(e) => {
                warn!("AI analysis failed: {}, proceeding with standard evaluation", e);
                self.evaluate_all_strategies(recent_quotes).await;
            }
        }
    }

    /// Evaluate all strategies with AI insights
    async fn evaluate_strategies_with_ai_insights(
        &self,
        recent_quotes: &[QuoteData],
        ai_response: &crate::agent::types::AIAnalysisResponse,
    ) {
        let market_conditions = self.market_conditions.read().await;
        let positions = self.current_positions.read().await;

        // Prioritize strategies based on AI recommendation
        let mut strategy_priority: Vec<_> = self.strategies.keys().collect();
        
        // Sort strategies based on AI confidence and market conditions
        strategy_priority.sort_by(|a, b| {
            let a_score = self.calculate_strategy_priority(a, ai_response);
            let b_score = self.calculate_strategy_priority(b, ai_response);
            b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Evaluate strategies in priority order
        for strategy_type in strategy_priority {
            if let (Some(strategy), Some(config)) = (
                self.strategies.get(&strategy_type),
                self.strategy_configs.get(&strategy_type)
            ) {
                // Evaluate strategy for most recent quotes
                for quote in recent_quotes.iter().rev().take(5) {
                    match strategy.evaluate(quote, &market_conditions, &positions, config).await {
                        Ok(Some(mut plan)) => {
                            // Enhance plan with AI insights
                            plan.execution_context.ai_reasoning = format!(
                                "{} AI confidence: {:.2}, Risk score: {:.2}",
                                ai_response.reasoning,
                                ai_response.confidence,
                                ai_response.risk_assessment.risk_score
                            );

                            // Adjust confidence based on AI assessment
                            plan.confidence_score *= ai_response.confidence;

                            info!("Generated {} plan with enhanced confidence {:.2}", 
                                  strategy_type.to_string(), plan.confidence_score);

                            if let Err(e) = self.plan_queue.send(plan) {
                                error!("Failed to send enhanced plan: {}", e);
                            }
                            
                            break; // Only generate one plan per strategy per evaluation cycle
                        }
                        Ok(None) => continue,
                        Err(e) => {
                            warn!("Error evaluating {:?}: {}", strategy_type, e);
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Evaluate all strategies without AI assistance
    async fn evaluate_all_strategies(&self, recent_quotes: &[QuoteData]) {
        let market_conditions = self.market_conditions.read().await;
        let positions = self.current_positions.read().await;

        for (strategy_type, strategy) in &self.strategies {
            if let Some(config) = self.strategy_configs.get(strategy_type) {
                for quote in recent_quotes.iter().rev().take(3) {
                    match strategy.evaluate(quote, &market_conditions, &positions, config).await {
                        Ok(Some(plan)) => {
                            info!("Standard plan generated: {:?}", strategy_type);
                            
                            if let Err(e) = self.plan_queue.send(plan) {
                                error!("Failed to send standard plan: {}", e);
                            }
                            break;
                        }
                        Ok(None) => continue,
                        Err(e) => {
                            warn!("Error in standard evaluation {:?}: {}", strategy_type, e);
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Calculate strategy priority score based on AI response
    fn calculate_strategy_priority(
        &self,
        strategy_type: &StrategyType,
        ai_response: &crate::agent::types::AIAnalysisResponse,
    ) -> f64 {
        let base_score = match strategy_type {
            StrategyType::Arbitrage => 0.9, // High priority for arbitrage
            StrategyType::GridTrading => 0.6,
            StrategyType::DCA => 0.5,
            StrategyType::MeanReversion => 0.7,
            StrategyType::TrendFollowing => 0.8,
        };

        // Adjust based on AI confidence and risk assessment
        let ai_adjustment = ai_response.confidence * (1.0 - ai_response.risk_assessment.risk_score);
        
        base_score * ai_adjustment
    }

    /// Update market conditions based on recent quotes
    async fn update_market_conditions(&self, recent_quotes: &[QuoteData]) -> Result<(), AgentError> {
        if recent_quotes.is_empty() {
            return Ok(());
        }

        // Calculate volatility from recent quotes
        let prices: Vec<f64> = recent_quotes.iter()
            .map(|q| if q.output_amount > 0 && q.input_amount > 0 {
                q.output_amount as f64 / q.input_amount as f64
            } else {
                1.0
            })
            .collect();

        let volatility = if prices.len() > 1 {
            let mean = prices.iter().sum::<f64>() / prices.len() as f64;
            let variance = prices.iter()
                .map(|price| (price - mean).powi(2))
                .sum::<f64>() / prices.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };

        // Estimate volume (simplified)
        let volume_24h = recent_quotes.iter()
            .map(|q| q.input_amount as f64)
            .sum::<f64>();

        // Determine price trend
        let price_trend = if prices.len() >= 2 {
            let first = prices[0];
            let last = prices[prices.len() - 1];
            let change_pct = (last - first) / first;
            
            if change_pct > 0.02 {
                crate::agent::types::PriceTrend::Bullish
            } else if change_pct < -0.02 {
                crate::agent::types::PriceTrend::Bearish
            } else {
                crate::agent::types::PriceTrend::Sideways
            }
        } else {
            crate::agent::types::PriceTrend::Sideways
        };

        // Calculate liquidity score based on spread and volume
        let avg_spread = recent_quotes.iter()
            .map(|q| q.price_impact_pct)
            .sum::<f64>() / recent_quotes.len() as f64;
        
        let liquidity_score = (1.0 - avg_spread.min(1.0)).max(0.0);

        let new_conditions = MarketConditions {
            volatility_24h: volatility,
            volume_24h,
            price_trend: price_trend.clone(),
            liquidity_score,
        };

        let mut market_conditions = self.market_conditions.write().await;
        *market_conditions = new_conditions;

        debug!("Updated market conditions: vol={:.4}, vol24h={:.0}, trend={:?}, liq={:.3}",
               volatility, volume_24h, price_trend, liquidity_score);

        Ok(())
    }

    /// Default market conditions
    fn default_market_conditions() -> MarketConditions {
        MarketConditions {
            volatility_24h: 0.05,
            volume_24h: 1_000_000.0,
            price_trend: crate::agent::types::PriceTrend::Sideways,
            liquidity_score: 0.5,
        }
    }

    /// Default strategy configuration
    fn default_strategy_config(&self) -> StrategyConfig {
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

    /// Default performance metrics
    fn default_performance_metrics(&self) -> crate::agent::types::PerformanceMetrics {
        use crate::agent::types::PerformanceMetrics;
        use rust_decimal::Decimal;
        use chrono::Utc;

        PerformanceMetrics {
            total_trades: 0,
            successful_trades: 0,
            total_pnl: Decimal::new(0, 0),
            win_rate: 0.0,
            avg_slippage_bps: 50.0,
            avg_execution_time_ms: 2000,
            max_drawdown: 0.0,
            sharpe_ratio: 0.0,
            last_updated: Utc::now(),
        }
    }

    /// Get planner statistics
    pub async fn get_stats(&self) -> PlannerStats {
        PlannerStats {
            is_active: *self.is_active.read().await,
            active_strategies: self.strategies.len(),
            current_positions: self.current_positions.read().await.len(),
            market_conditions: self.market_conditions.read().await.clone(),
        }
    }
}

// Add trait implementation for StrategyType
impl StrategyType {
    fn to_string(&self) -> String {
        match self {
            StrategyType::Arbitrage => "Arbitrage".to_string(),
            StrategyType::GridTrading => "GridTrading".to_string(),
            StrategyType::DCA => "DCA".to_string(),
            StrategyType::MeanReversion => "MeanReversion".to_string(),
            StrategyType::TrendFollowing => "TrendFollowing".to_string(),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct PlannerStats {
    pub is_active: bool,
    pub active_strategies: usize,
    pub current_positions: usize,
    pub market_conditions: MarketConditions,
}
