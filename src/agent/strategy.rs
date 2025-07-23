use std::collections::HashMap;
use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use tracing::{info, warn, debug};

use crate::agent::types::{
    QuoteData, TradingPlan, StrategyConfig, StrategyType, StrategyParameters,
    RiskLimits, ExecutionSettings, MarketConditions, Position, AgentError,
    ExecutionContext, RiskAssessment, PriceTrend,
};

#[async_trait]
pub trait Strategy: Send + Sync {
    /// Evaluate market data and generate trading plan if conditions are met
    async fn evaluate(
        &self,
        quote: &QuoteData,
        market_conditions: &MarketConditions,
        current_positions: &HashMap<String, Position>,
        config: &StrategyConfig,
    ) -> Result<Option<TradingPlan>, AgentError>;

    /// Get strategy type
    fn strategy_type(&self) -> StrategyType;

    /// Validate strategy parameters
    fn validate_parameters(&self, params: &StrategyParameters) -> Result<(), AgentError>;
}

/// Arbitrage strategy implementation
pub struct ArbitrageStrategy;

#[async_trait]
impl Strategy for ArbitrageStrategy {
    async fn evaluate(
        &self,
        quote: &QuoteData,
        market_conditions: &MarketConditions,
        current_positions: &HashMap<String, Position>,
        config: &StrategyConfig,
    ) -> Result<Option<TradingPlan>, AgentError> {
        // Calculate effective spread considering fees and slippage
        let spread_bps = self.calculate_effective_spread(quote)?;
        
        // Check if spread meets minimum threshold
        if spread_bps < config.parameters.min_spread_bps {
            debug!(
                "Spread {} bps below minimum {} bps for {}/{}",
                spread_bps,
                config.parameters.min_spread_bps,
                quote.input_mint,
                quote.output_mint
            );
            return Ok(None);
        }

        // Check market conditions
        if !self.check_market_conditions(market_conditions, config)? {
            return Ok(None);
        }

        // Check risk limits
        self.check_risk_limits(current_positions, config)?;

        // Generate trading plan
        let plan = self.create_trading_plan(quote, config, spread_bps).await?;
        
        info!(
            "Arbitrage opportunity detected: spread {} bps, confidence {}",
            spread_bps,
            plan.confidence_score
        );

        Ok(Some(plan))
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::Arbitrage
    }

    fn validate_parameters(&self, params: &StrategyParameters) -> Result<(), AgentError> {
        if params.min_spread_bps < 10 {
            return Err(AgentError::Configuration("Minimum spread too low for arbitrage".to_string()));
        }
        if params.max_slippage_bps > 500 {
            return Err(AgentError::Configuration("Maximum slippage too high".to_string()));
        }
        Ok(())
    }
}

impl ArbitrageStrategy {
    pub fn new() -> Self {
        Self
    }

    fn calculate_effective_spread(&self, quote: &QuoteData) -> Result<u16, AgentError> {
        if quote.input_amount == 0 || quote.output_amount == 0 {
            return Ok(0);
        }

        // Calculate price impact and fees
        let price_impact_bps = (quote.price_impact_pct * 100.0) as u16;
        let total_fees_bps = quote.slippage_bps + quote.platform_fee_bps + price_impact_bps;

        // Calculate raw spread (simplified)
        let input_price = quote.input_amount as f64;
        let output_price = quote.output_amount as f64;
        let raw_spread = ((output_price / input_price - 1.0) * 10000.0) as u16;

        // Effective spread after costs
        let effective_spread = raw_spread.saturating_sub(total_fees_bps);

        Ok(effective_spread)
    }

    fn check_market_conditions(
        &self,
        conditions: &MarketConditions,
        config: &StrategyConfig,
    ) -> Result<bool, AgentError> {
        // Avoid trading in extreme volatility
        if conditions.volatility_24h > 0.15 {
            debug!("High volatility {}, avoiding arbitrage", conditions.volatility_24h);
            return Ok(false);
        }

        // Require minimum liquidity
        if conditions.liquidity_score < 0.3 {
            debug!("Low liquidity score {}", conditions.liquidity_score);
            return Ok(false);
        }

        Ok(true)
    }

    fn check_risk_limits(
        &self,
        positions: &HashMap<String, Position>,
        config: &StrategyConfig,
    ) -> Result<(), AgentError> {
        let total_position_value: f64 = positions.values()
            .map(|p| p.amount as f64 * p.current_price)
            .sum();

        if total_position_value > config.risk_limits.max_position_size_usd {
            return Err(AgentError::RiskLimitExceeded(
                format!("Total position ${:.2} exceeds limit ${:.2}",
                    total_position_value,
                    config.risk_limits.max_position_size_usd
                )
            ));
        }

        Ok(())
    }

    async fn create_trading_plan(
        &self,
        quote: &QuoteData,
        config: &StrategyConfig,
        spread_bps: u16,
    ) -> Result<TradingPlan, AgentError> {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        let input_mint = Pubkey::from_str(&quote.input_mint)
            .map_err(|e| AgentError::Configuration(format!("Invalid input mint: {}", e)))?;
        
        let output_mint = Pubkey::from_str(&quote.output_mint)
            .map_err(|e| AgentError::Configuration(format!("Invalid output mint: {}", e)))?;

        // Calculate position size based on strategy config
        let position_size = (config.parameters.position_size_usd * 1_000_000.0) as u64; // Convert to lamports/tokens

        // Calculate confidence based on spread and market conditions
        let confidence = self.calculate_confidence(spread_bps, config);

        let plan = TradingPlan {
            id: uuid::Uuid::new_v4(),
            strategy_type: StrategyType::Arbitrage,
            bucket_pubkey: input_mint, // Placeholder - should be actual bucket
            input_mint,
            output_mint,
            input_amount: position_size,
            min_output_amount: quote.output_amount,
            max_slippage_bps: config.parameters.max_slippage_bps,
            priority_fee: self.calculate_priority_fee(&config.execution_settings),
            route_plan: self.encode_route_plan(&quote.route_plan)?,
            confidence_score: confidence,
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::seconds(30), // Short expiry for arbitrage
            execution_context: ExecutionContext {
                market_conditions: MarketConditions {
                    volatility_24h: 0.0, // Will be filled by caller
                    volume_24h: 0.0,
                    price_trend: PriceTrend::Sideways,
                    liquidity_score: 0.5,
                },
                risk_assessment: RiskAssessment {
                    risk_score: 0.3, // Arbitrage is generally low risk
                    max_loss_estimate: (spread_bps as f64 * position_size as f64 / 10000.0),
                    position_risk_pct: 5.0,
                    market_risk_factors: vec!["slippage".to_string(), "timing".to_string()],
                },
                ai_reasoning: format!("Arbitrage opportunity with {}bps spread", spread_bps),
            },
        };

        Ok(plan)
    }

    fn calculate_confidence(&self, spread_bps: u16, config: &StrategyConfig) -> f64 {
        let min_spread = config.parameters.min_spread_bps as f64;
        let spread_ratio = spread_bps as f64 / min_spread;
        
        // Higher spread = higher confidence, capped at 0.9
        (0.5 + (spread_ratio - 1.0) * 0.2).min(0.9)
    }

    fn calculate_priority_fee(&self, settings: &ExecutionSettings) -> u64 {
        // Use configured priority fee with some randomization for MEV protection
        let base_fee = settings.max_priority_fee_lamports;
        let jitter = (base_fee as f64 * 0.1) as u64;
        base_fee + (rand::random::<u64>() % jitter)
    }

    fn encode_route_plan(&self, route_plan: &[crate::agent::types::RoutePlan]) -> Result<Vec<u8>, AgentError> {
        // Serialize route plan for Jupiter swap instruction
        bincode::serialize(route_plan)
            .map_err(|e| AgentError::Configuration(format!("Failed to encode route plan: {}", e)))
    }
}

/// Grid trading strategy implementation
pub struct GridTradingStrategy {
    grid_levels: usize,
    grid_spacing_pct: f64,
}

#[async_trait]
impl Strategy for GridTradingStrategy {
    async fn evaluate(
        &self,
        quote: &QuoteData,
        market_conditions: &MarketConditions,
        current_positions: &HashMap<String, Position>,
        config: &StrategyConfig,
    ) -> Result<Option<TradingPlan>, AgentError> {
        // Grid trading logic - check if price hits grid levels
        match market_conditions.price_trend {
            PriceTrend::Sideways => {
                // Ideal conditions for grid trading
                self.evaluate_grid_opportunity(quote, current_positions, config).await
            },
            _ => {
                // Not suitable for trending markets
                debug!("Market trending, skipping grid strategy");
                Ok(None)
            }
        }
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::GridTrading
    }

    fn validate_parameters(&self, params: &StrategyParameters) -> Result<(), AgentError> {
        if params.rebalance_threshold_pct < 0.01 || params.rebalance_threshold_pct > 0.1 {
            return Err(AgentError::Configuration("Invalid rebalance threshold for grid trading".to_string()));
        }
        Ok(())
    }
}

impl GridTradingStrategy {
    pub fn new(grid_levels: usize, grid_spacing_pct: f64) -> Self {
        Self {
            grid_levels,
            grid_spacing_pct,
        }
    }

    async fn evaluate_grid_opportunity(
        &self,
        quote: &QuoteData,
        current_positions: &HashMap<String, Position>,
        config: &StrategyConfig,
    ) -> Result<Option<TradingPlan>, AgentError> {
        // Simplified grid logic - would need more sophisticated implementation
        // Check if current price deviates enough from average to trigger rebalance
        
        let pair_key = format!("{}_{}", quote.input_mint, quote.output_mint);
        if let Some(position) = current_positions.get(&pair_key) {
            let price_deviation = ((position.current_price / position.entry_price - 1.0).abs() * 100.0) as u16;
            
            if price_deviation > (config.parameters.rebalance_threshold_pct * 100.0) as u16 {
                info!("Grid rebalance triggered: {}% deviation", price_deviation as f64 / 100.0);
                // Would generate rebalancing plan here
            }
        }

        Ok(None) // Simplified - return None for now
    }
}

/// DCA (Dollar Cost Averaging) strategy
pub struct DCAStrategy {
    interval_hours: u32,
    last_execution: HashMap<String, DateTime<Utc>>,
}

#[async_trait]
impl Strategy for DCAStrategy {
    async fn evaluate(
        &self,
        quote: &QuoteData,
        market_conditions: &MarketConditions,
        current_positions: &HashMap<String, Position>,
        config: &StrategyConfig,
    ) -> Result<Option<TradingPlan>, AgentError> {
        let pair_key = format!("{}_{}", quote.input_mint, quote.output_mint);
        
        // Check if enough time has passed since last DCA
        if let Some(last_exec) = self.last_execution.get(&pair_key) {
            let time_since = Utc::now().signed_duration_since(*last_exec);
            if time_since.num_hours() < self.interval_hours as i64 {
                return Ok(None);
            }
        }

        // DCA regardless of market conditions (that's the point)
        self.create_dca_plan(quote, config).await.map(Some)
    }

    fn strategy_type(&self) -> StrategyType {
        StrategyType::DCA
    }

    fn validate_parameters(&self, _params: &StrategyParameters) -> Result<(), AgentError> {
        Ok(()) // DCA has minimal parameter requirements
    }
}

impl DCAStrategy {
    pub fn new(interval_hours: u32) -> Self {
        Self {
            interval_hours,
            last_execution: HashMap::new(),
        }
    }

    async fn create_dca_plan(
        &self,
        quote: &QuoteData,
        config: &StrategyConfig,
    ) -> Result<TradingPlan, AgentError> {
        use solana_sdk::pubkey::Pubkey;
        use std::str::FromStr;

        let input_mint = Pubkey::from_str(&quote.input_mint)?;
        let output_mint = Pubkey::from_str(&quote.output_mint)?;

        let plan = TradingPlan {
            id: uuid::Uuid::new_v4(),
            strategy_type: StrategyType::DCA,
            bucket_pubkey: input_mint,
            input_mint,
            output_mint,
            input_amount: (config.parameters.position_size_usd * 1_000_000.0) as u64,
            min_output_amount: (quote.output_amount as f64 * 0.95) as u64, // 5% slippage tolerance
            max_slippage_bps: config.parameters.max_slippage_bps,
            priority_fee: config.execution_settings.max_priority_fee_lamports / 2, // Lower priority for DCA
            route_plan: bincode::serialize(&quote.route_plan)?,
            confidence_score: 0.8, // DCA has consistent confidence
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(1), // Longer expiry for DCA
            execution_context: ExecutionContext {
                market_conditions: MarketConditions {
                    volatility_24h: 0.0,
                    volume_24h: 0.0,
                    price_trend: PriceTrend::Sideways,
                    liquidity_score: 0.5,
                },
                risk_assessment: RiskAssessment {
                    risk_score: 0.2, // DCA is low risk
                    max_loss_estimate: config.parameters.position_size_usd * 0.1, // 10% max loss estimate
                    position_risk_pct: 2.0,
                    market_risk_factors: vec!["timing".to_string()],
                },
                ai_reasoning: "Regular DCA execution regardless of market conditions".to_string(),
            },
        };

        Ok(plan)
    }
}

/// Strategy factory for creating strategy instances
pub struct StrategyFactory;

impl StrategyFactory {
    pub fn create_strategy(strategy_type: StrategyType) -> Box<dyn Strategy> {
        match strategy_type {
            StrategyType::Arbitrage => Box::new(ArbitrageStrategy::new()),
            StrategyType::GridTrading => Box::new(GridTradingStrategy::new(10, 0.02)),
            StrategyType::DCA => Box::new(DCAStrategy::new(24)), // 24 hour intervals
            StrategyType::MeanReversion => Box::new(ArbitrageStrategy::new()), // Placeholder
            StrategyType::TrendFollowing => Box::new(ArbitrageStrategy::new()), // Placeholder
        }
    }

    pub fn validate_strategy_config(config: &StrategyConfig) -> Result<(), AgentError> {
        let strategy = Self::create_strategy(config.strategy_type.clone());
        strategy.validate_parameters(&config.parameters)
    }
}
