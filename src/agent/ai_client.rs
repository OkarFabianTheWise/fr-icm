use std::collections::HashMap;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{info, warn, error};
use chrono::Utc;

use crate::agent::types::{
    AIAnalysisRequest, AIAnalysisResponse, TradingRecommendation,
    QuoteData, Position, StrategyConfig, PerformanceMetrics,
    RiskAssessment, MarketConditions, PriceTrend, AgentError,
};

pub struct AIClient {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

impl AIClient {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_key,
            model: "gpt-4-turbo-preview".to_string(),
            max_tokens: 4000,
            temperature: 0.3,
        }
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Analyze market conditions and generate trading recommendations
    pub async fn analyze_trading_opportunity(
        &self,
        request: AIAnalysisRequest,
    ) -> Result<AIAnalysisResponse, AgentError> {
        let system_prompt = self.create_system_prompt();
        let user_prompt = self.create_analysis_prompt(&request);

        let response = self.call_openai_api(&system_prompt, &user_prompt).await?;
        
        // Parse the structured response from AI
        self.parse_ai_response(&response).await
    }

    /// Get market sentiment analysis
    pub async fn get_market_sentiment(
        &self,
        market_data: &[QuoteData],
        token_symbols: &HashMap<String, String>,
    ) -> Result<MarketConditions, AgentError> {
        let prompt = format!(
            r#"Analyze the following market data and provide sentiment analysis:

Market Data:
{}

Token Symbols:
{}

Provide analysis in the following JSON format:
{{
    "volatility_24h": <number>,
    "volume_24h": <number>,
    "price_trend": "Bullish|Bearish|Sideways",
    "liquidity_score": <number 0-1>,
    "analysis_summary": "<brief summary>"
}}
"#,
            serde_json::to_string_pretty(market_data)
                .map_err(|e| AgentError::Serialization(e))?,
            serde_json::to_string_pretty(token_symbols)
                .map_err(|e| AgentError::Serialization(e))?
        );

        let response = self.call_openai_api(
            "You are a cryptocurrency market analyst. Analyze market data and provide sentiment.",
            &prompt
        ).await?;

        self.parse_market_conditions(&response).await
    }

    /// Get risk assessment for a potential trade
    pub async fn assess_trade_risk(
        &self,
        quote: &QuoteData,
        current_position: Option<&Position>,
        strategy: &StrategyConfig,
        market_conditions: &MarketConditions,
    ) -> Result<RiskAssessment, AgentError> {
        let prompt = format!(
            r#"Assess the risk for this potential trade:

Quote Data:
{}

Current Position:
{}

Strategy Configuration:
{}

Market Conditions:
{}

Provide risk assessment in JSON format:
{{
    "risk_score": <number 0-1>,
    "max_loss_estimate": <number>,
    "position_risk_pct": <number>,
    "market_risk_factors": ["factor1", "factor2", ...],
    "recommendation": "PROCEED|CAUTION|ABORT",
    "reasoning": "<detailed reasoning>"
}}
"#,
            serde_json::to_string_pretty(quote)
                .map_err(|e| AgentError::Serialization(e))?,
            match current_position {
                Some(pos) => serde_json::to_string_pretty(pos)
                    .map_err(|e| AgentError::Serialization(e))?,
                None => "No current position".to_string(),
            },
            serde_json::to_string_pretty(strategy)
                .map_err(|e| AgentError::Serialization(e))?,
            serde_json::to_string_pretty(market_conditions)
                .map_err(|e| AgentError::Serialization(e))?
        );

        let response = self.call_openai_api(
            "You are a risk management specialist for cryptocurrency trading. Assess trade risk carefully.",
            &prompt
        ).await?;

        self.parse_risk_assessment(&response).await
    }

    /// Generate strategy optimization suggestions
    pub async fn optimize_strategy(
        &self,
        current_strategy: &StrategyConfig,
        performance: &PerformanceMetrics,
        recent_trades: &[Position],
    ) -> Result<HashMap<String, f64>, AgentError> {
        let prompt = format!(
            r#"Analyze the current trading strategy performance and suggest optimizations:

Current Strategy:
{}

Performance Metrics:
{}

Recent Trades:
{}

Provide optimization suggestions in JSON format:
{{
    "suggested_parameters": {{
        "min_spread_bps": <number>,
        "max_slippage_bps": <number>,
        "position_size_usd": <number>,
        "rebalance_threshold_pct": <number>,
        "priority_fee_percentile": <number>
    }},
    "reasoning": "<explanation of changes>",
    "expected_improvement": "<expected performance improvement>"
}}
"#,
            serde_json::to_string_pretty(current_strategy)
                .map_err(|e| AgentError::Serialization(e))?,
            serde_json::to_string_pretty(performance)
                .map_err(|e| AgentError::Serialization(e))?,
            serde_json::to_string_pretty(recent_trades)
                .map_err(|e| AgentError::Serialization(e))?
        );

        let response = self.call_openai_api(
            "You are a quantitative trading strategist. Optimize trading strategies based on performance data.",
            &prompt
        ).await?;

        self.parse_optimization_response(&response).await
    }

    /// Create the system prompt for the AI
    fn create_system_prompt(&self) -> String {
        r#"You are an advanced cryptocurrency trading AI assistant with expertise in:
- Solana blockchain and DeFi protocols
- Jupiter DEX aggregator and swap mechanics
- Risk management and portfolio optimization
- Market microstructure and liquidity analysis
- Quantitative trading strategies

Your role is to analyze market data, assess risks, and provide actionable trading recommendations.
Always consider:
1. Market conditions and volatility
2. Liquidity and slippage impacts
3. Risk management principles
4. Transaction costs and fees
5. Position sizing and capital allocation

Respond with structured JSON data when requested, and provide clear reasoning for all recommendations.
Be conservative with risk assessment and prioritize capital preservation."#.to_string()
    }

    /// Create the analysis prompt for trading decisions
    fn create_analysis_prompt(&self, request: &AIAnalysisRequest) -> String {
        format!(
            r#"Analyze the following trading scenario and provide a recommendation:

MARKET DATA:
{}

CURRENT POSITIONS:
{}

STRATEGY CONFIGURATION:
{}

PERFORMANCE HISTORY:
{}

SPECIFIC QUESTION:
{}

Please provide your analysis in the following JSON format:
{{
    "recommendation": {{
        "action": "Buy|Sell|Hold|Rebalance|StopLoss",
        "amount": <number if applicable>,
        "target_price": <number if applicable>,
        "adjustments": {{}} // for rebalance actions
    }},
    "reasoning": "<detailed explanation of your recommendation>",
    "confidence": <number 0-1>,
    "risk_assessment": {{
        "risk_score": <number 0-1>,
        "max_loss_estimate": <number>,
        "position_risk_pct": <number>,
        "market_risk_factors": ["factor1", "factor2"]
    }},
    "suggested_parameters": {{
        "parameter_name": <suggested_value>
    }}
}}

Consider market conditions, volatility, liquidity, and risk management in your analysis.
"#,
            serde_json::to_string_pretty(&request.market_data).unwrap_or_default(),
            serde_json::to_string_pretty(&request.current_positions).unwrap_or_default(),
            serde_json::to_string_pretty(&request.strategy_config).unwrap_or_default(),
            serde_json::to_string_pretty(&request.performance_history).unwrap_or_default(),
            request.question
        )
    }

    /// Call the OpenAI API
    async fn call_openai_api(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, AgentError> {
        let payload = json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": user_prompt
                }
            ],
            "max_tokens": self.max_tokens,
            "temperature": self.temperature,
            "response_format": { "type": "json_object" }
        });

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(AgentError::AIAnalysis(format!(
                "OpenAI API error {}: {}",
                status, error_text
            )));
        }

        let json: Value = response.json().await?;
        
        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| AgentError::AIAnalysis("No content in OpenAI response".to_string()))?;

        Ok(content.to_string())
    }

    /// Parse AI response into structured format
    async fn parse_ai_response(&self, response: &str) -> Result<AIAnalysisResponse, AgentError> {
        let json: Value = serde_json::from_str(response)
            .map_err(|e| AgentError::AIAnalysis(format!("Failed to parse AI response: {}", e)))?;

        let recommendation = self.parse_recommendation(&json["recommendation"])?;
        
        let risk_assessment = RiskAssessment {
            risk_score: json["risk_assessment"]["risk_score"].as_f64().unwrap_or(0.5),
            max_loss_estimate: json["risk_assessment"]["max_loss_estimate"].as_f64().unwrap_or(0.0),
            position_risk_pct: json["risk_assessment"]["position_risk_pct"].as_f64().unwrap_or(0.0),
            market_risk_factors: json["risk_assessment"]["market_risk_factors"]
                .as_array()
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect())
                .unwrap_or_default(),
        };

        let suggested_parameters = json.get("suggested_parameters")
            .and_then(|p| p.as_object())
            .map(|obj| obj.iter()
                .filter_map(|(k, v)| v.as_f64().map(|val| (k.clone(), val)))
                .collect());

        Ok(AIAnalysisResponse {
            recommendation,
            reasoning: json["reasoning"].as_str().unwrap_or("").to_string(),
            confidence: json["confidence"].as_f64().unwrap_or(0.5),
            risk_assessment,
            suggested_parameters,
        })
    }

    /// Parse trading recommendation from JSON
    fn parse_recommendation(&self, json: &Value) -> Result<TradingRecommendation, AgentError> {
        let action = json["action"].as_str()
            .ok_or_else(|| AgentError::AIAnalysis("Missing action in recommendation".to_string()))?;

        match action {
            "Buy" => Ok(TradingRecommendation::Buy {
                amount: json["amount"].as_u64().unwrap_or(0),
                target_price: json["target_price"].as_f64().unwrap_or(0.0),
            }),
            "Sell" => Ok(TradingRecommendation::Sell {
                amount: json["amount"].as_u64().unwrap_or(0),
                target_price: json["target_price"].as_f64().unwrap_or(0.0),
            }),
            "Hold" => Ok(TradingRecommendation::Hold),
            "Rebalance" => {
                let adjustments = json["adjustments"].as_object()
                    .map(|obj| obj.iter()
                        .filter_map(|(k, v)| v.as_f64().map(|val| (k.clone(), val)))
                        .collect())
                    .unwrap_or_default();
                Ok(TradingRecommendation::Rebalance { adjustments })
            },
            "StopLoss" => Ok(TradingRecommendation::StopLoss),
            _ => Err(AgentError::AIAnalysis(format!("Unknown action: {}", action))),
        }
    }

    /// Parse market conditions from AI response
    async fn parse_market_conditions(&self, response: &str) -> Result<MarketConditions, AgentError> {
        let json: Value = serde_json::from_str(response)
            .map_err(|e| AgentError::AIAnalysis(format!("Failed to parse market conditions: {}", e)))?;

        let price_trend = match json["price_trend"].as_str() {
            Some("Bullish") => PriceTrend::Bullish,
            Some("Bearish") => PriceTrend::Bearish,
            _ => PriceTrend::Sideways,
        };

        Ok(MarketConditions {
            volatility_24h: json["volatility_24h"].as_f64().unwrap_or(0.0),
            volume_24h: json["volume_24h"].as_f64().unwrap_or(0.0),
            price_trend,
            liquidity_score: json["liquidity_score"].as_f64().unwrap_or(0.5),
        })
    }

    /// Parse risk assessment from AI response
    async fn parse_risk_assessment(&self, response: &str) -> Result<RiskAssessment, AgentError> {
        let json: Value = serde_json::from_str(response)
            .map_err(|e| AgentError::AIAnalysis(format!("Failed to parse risk assessment: {}", e)))?;

        Ok(RiskAssessment {
            risk_score: json["risk_score"].as_f64().unwrap_or(0.5),
            max_loss_estimate: json["max_loss_estimate"].as_f64().unwrap_or(0.0),
            position_risk_pct: json["position_risk_pct"].as_f64().unwrap_or(0.0),
            market_risk_factors: json["market_risk_factors"]
                .as_array()
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect())
                .unwrap_or_default(),
        })
    }

    /// Parse optimization suggestions from AI response
    async fn parse_optimization_response(&self, response: &str) -> Result<HashMap<String, f64>, AgentError> {
        let json: Value = serde_json::from_str(response)
            .map_err(|e| AgentError::AIAnalysis(format!("Failed to parse optimization response: {}", e)))?;

        let suggested_parameters = json.get("suggested_parameters")
            .and_then(|p| p.as_object())
            .map(|obj| obj.iter()
                .filter_map(|(k, v)| v.as_f64().map(|val| (k.clone(), val)))
                .collect())
            .unwrap_or_default();

        Ok(suggested_parameters)
    }
}
