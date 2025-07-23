use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, Instant};
use reqwest::Client;
use serde_json::Value;
use chrono::Utc;
use dashmap::DashMap;
use tracing::{info, warn, error, debug};

use crate::agent::types::{QuoteData, RoutePlan, SwapInfo, AgentError};

const JUPITER_QUOTE_API: &str = "https://quote-api.jup.ag/v6";
const JUPITER_PRICE_API: &str = "https://api.jup.ag/price/v2";

/// Data fetcher for continuous market data acquisition
pub struct DataFetcher {
    client: Client,
    quote_cache: Arc<DashMap<String, QuoteData>>,
    price_cache: Arc<DashMap<String, f64>>,
    token_pairs: Vec<(String, String)>,
    fetch_interval: Duration,
    quote_sender: mpsc::UnboundedSender<QuoteData>,
    is_running: Arc<RwLock<bool>>,
}

impl DataFetcher {
    pub fn new(
        token_pairs: Vec<(String, String)>,
        fetch_interval_ms: u64,
    ) -> (Self, mpsc::UnboundedReceiver<QuoteData>) {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        let (quote_sender, quote_receiver) = mpsc::unbounded_channel();

        let fetcher = Self {
            client,
            quote_cache: Arc::new(DashMap::new()),
            price_cache: Arc::new(DashMap::new()),
            token_pairs,
            fetch_interval: Duration::from_millis(fetch_interval_ms),
            quote_sender,
            is_running: Arc::new(RwLock::new(false)),
        };

        (fetcher, quote_receiver)
    }

    /// Start the continuous data fetching loop
    pub async fn start(&self) -> Result<(), AgentError> {
        {
            let mut is_running = self.is_running.write().await;
            if *is_running {
                return Ok(());
            }
            *is_running = true;
        }

        info!("Starting data fetcher with {} token pairs", self.token_pairs.len());

        let mut interval = interval(self.fetch_interval);
        
        while *self.is_running.read().await {
            interval.tick().await;
            
            let start_time = Instant::now();
            
            // Fetch quotes for all token pairs concurrently
            let fetch_tasks: Vec<_> = self.token_pairs
                .iter()
                .map(|(input, output)| {
                    self.fetch_quote_for_pair(input.clone(), output.clone())
                })
                .collect();

            let results = futures::future::join_all(fetch_tasks).await;
            
            let mut successful_fetches = 0;
            for result in results {
                match result {
                    Ok(quote) => {
                        let cache_key = format!("{}_{}", quote.input_mint, quote.output_mint);
                        self.quote_cache.insert(cache_key, quote.clone());
                        
                        if let Err(e) = self.quote_sender.send(quote) {
                            warn!("Failed to send quote to channel: {}", e);
                        } else {
                            successful_fetches += 1;
                        }
                    }
                    Err(e) => {
                        error!("Failed to fetch quote: {}", e);
                    }
                }
            }

            let fetch_duration = start_time.elapsed();
            debug!(
                "Fetched {}/{} quotes in {:?}",
                successful_fetches,
                self.token_pairs.len(),
                fetch_duration
            );

            // Update prices separately for better performance
            if let Err(e) = self.update_token_prices().await {
                warn!("Failed to update token prices: {}", e);
            }
        }

        info!("Data fetcher stopped");
        Ok(())
    }

    /// Stop the data fetcher
    pub async fn stop(&self) {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        info!("Data fetcher stop signal sent");
    }

    /// Fetch quote for a specific token pair
    async fn fetch_quote_for_pair(
        &self,
        input_mint: String,
        output_mint: String,
    ) -> Result<QuoteData, AgentError> {
        let amount = 1_000_000; // 1 token in smallest units for price discovery
        
        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps=50",
            JUPITER_QUOTE_API, input_mint, output_mint, amount
        );

        let response = self.client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AgentError::JupiterApi(format!(
                "HTTP {} for {}/{}",
                response.status(),
                input_mint,
                output_mint
            )));
        }

        let json: Value = response.json().await?;
        
        // Parse the Jupiter quote response
        let quote = QuoteData {
            input_mint: json["inputMint"]
                .as_str()
                .unwrap_or(&input_mint)
                .to_string(),
            output_mint: json["outputMint"]
                .as_str()
                .unwrap_or(&output_mint)
                .to_string(),
            input_amount: json["inAmount"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(amount),
            output_amount: json["outAmount"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            other_amount_threshold: json["otherAmountThreshold"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            swap_mode: json["swapMode"]
                .as_str()
                .unwrap_or("ExactIn")
                .to_string(),
            slippage_bps: json["slippageBps"]
                .as_u64()
                .unwrap_or(50) as u16,
            platform_fee_bps: json["platformFeeBps"]
                .as_u64()
                .unwrap_or(0) as u16,
            price_impact_pct: json["priceImpactPct"]
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            route_plan: self.parse_route_plan(&json["routePlan"])?,
            timestamp: Utc::now(),
        };

        Ok(quote)
    }

    /// Parse Jupiter route plan from JSON
    fn parse_route_plan(&self, route_plan_json: &Value) -> Result<Vec<RoutePlan>, AgentError> {
        let mut route_plans = Vec::new();

        if let Some(routes) = route_plan_json.as_array() {
            for route in routes {
                if let Some(swap_info_json) = route.get("swapInfo") {
                    let swap_info = SwapInfo {
                        amm_key: swap_info_json["ammKey"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        label: swap_info_json["label"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        input_mint: swap_info_json["inputMint"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        output_mint: swap_info_json["outputMint"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        in_amount: swap_info_json["inAmount"]
                            .as_str()
                            .unwrap_or("0")
                            .to_string(),
                        out_amount: swap_info_json["outAmount"]
                            .as_str()
                            .unwrap_or("0")
                            .to_string(),
                        fee_amount: swap_info_json["feeAmount"]
                            .as_str()
                            .unwrap_or("0")
                            .to_string(),
                        fee_mint: swap_info_json["feeMint"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                    };

                    let route_plan = RoutePlan {
                        swap_info,
                        percent: route["percent"]
                            .as_u64()
                            .unwrap_or(100) as u8,
                    };

                    route_plans.push(route_plan);
                }
            }
        }

        Ok(route_plans)
    }

    /// Update token prices from Jupiter Price API
    async fn update_token_prices(&self) -> Result<(), AgentError> {
        // Collect unique token mints
        let mut unique_tokens = std::collections::HashSet::new();
        for (input, output) in &self.token_pairs {
            unique_tokens.insert(input.clone());
            unique_tokens.insert(output.clone());
        }

        let tokens: Vec<String> = unique_tokens.into_iter().collect();
        if tokens.is_empty() {
            return Ok(());
        }

        let ids = tokens.join(",");
        let url = format!("{}?ids={}", JUPITER_PRICE_API, ids);

        let response = self.client.get(&url).send().await?;
        
        if !response.status().is_success() {
            return Err(AgentError::JupiterApi(format!(
                "Price API HTTP {}",
                response.status()
            )));
        }

        let json: Value = response.json().await?;
        
        if let Some(data) = json.get("data").and_then(|d| d.as_object()) {
            for (token, price_data) in data {
                if let Some(price) = price_data.get("price").and_then(|p| p.as_f64()) {
                    self.price_cache.insert(token.clone(), price);
                }
            }
        }

        Ok(())
    }

    /// Get cached quote for a token pair
    pub fn get_cached_quote(&self, input_mint: &str, output_mint: &str) -> Option<QuoteData> {
        let cache_key = format!("{}_{}", input_mint, output_mint);
        self.quote_cache.get(&cache_key).map(|entry| entry.value().clone())
    }

    /// Get cached price for a token
    pub fn get_cached_price(&self, token_mint: &str) -> Option<f64> {
        self.price_cache.get(token_mint).map(|entry| *entry.value())
    }

    /// Get all cached quotes
    pub fn get_all_cached_quotes(&self) -> HashMap<String, QuoteData> {
        self.quote_cache
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Check if data is fresh (within configured interval)
    pub fn is_data_fresh(&self, quote: &QuoteData) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(quote.timestamp);
        age.num_milliseconds() < (self.fetch_interval.as_millis() as i64 * 3)
    }

    /// Get statistics about the data fetcher
    pub async fn get_stats(&self) -> DataFetcherStats {
        DataFetcherStats {
            is_running: *self.is_running.read().await,
            cached_quotes: self.quote_cache.len(),
            cached_prices: self.price_cache.len(),
            configured_pairs: self.token_pairs.len(),
            fetch_interval_ms: self.fetch_interval.as_millis() as u64,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct DataFetcherStats {
    pub is_running: bool,
    pub cached_quotes: usize,
    pub cached_prices: usize,
    pub configured_pairs: usize,
    pub fetch_interval_ms: u64,
}
