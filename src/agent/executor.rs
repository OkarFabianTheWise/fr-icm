use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc, Semaphore};
use tokio::time::{timeout, Instant};
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{info, warn, error, debug};
use chrono::Utc;
use anchor_lang::prelude::*;
use crate::agent::types::{TradingPlan, AgentError, ExecutionSettings};
use crate::state_structs::{SwapTokensRequest, UnsignedTransactionResponse};
use crate::onchain_instance::instance::IcmProgramInstance;
use solana_sdk::signature::{
    read_keypair_file,
};
use std::str::FromStr;

use spl_token_2022::ID as TOKEN_2022_PROGRAM_ID;
use std::result::Result as StdResult;

const JUPITER_SWAP_API: &str = "https://quote-api.jup.ag/v6";

/// Executes trading plans by building and submitting transactions
#[derive(Debug)]
pub struct Executor {
    icm_client: Arc<IcmProgramInstance>,
    http_client: Client,
    execution_semaphore: Arc<Semaphore>,
    plan_receiver: Option<mpsc::UnboundedReceiver<TradingPlan>>,
    execution_results: mpsc::UnboundedSender<ExecutionResult>,
    is_active: Arc<RwLock<bool>>,
    metrics: Arc<RwLock<ExecutionMetrics>>,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub plan_id: uuid::Uuid,
    pub success: bool,
    pub transaction_signature: Option<String>,
    pub execution_time_ms: u64,
    pub actual_slippage_bps: Option<u16>,
    pub error_message: Option<String>,
    pub gas_used: Option<u64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Default, Clone)]
pub struct ExecutionMetrics {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub avg_execution_time_ms: u64,
    pub total_gas_used: u64,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
}

impl Executor {

    pub async fn start_with_receiver(&mut self, plan_receiver: mpsc::UnboundedReceiver<TradingPlan>) -> StdResult<(), AgentError> {
        self.plan_receiver = Some(plan_receiver);
        // You can add your execution loop logic here
        Ok(())
    }
    pub fn new(
        icm_client: Arc<IcmProgramInstance>,
        max_concurrent_executions: usize,
    ) -> (Self, mpsc::UnboundedReceiver<TradingPlan>, mpsc::UnboundedReceiver<ExecutionResult>) {
        let (plan_sender, plan_receiver) = mpsc::unbounded_channel();
        let (result_sender, result_receiver) = mpsc::unbounded_channel();

        let http_client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        let executor = Self {
            icm_client,
            http_client,
            execution_semaphore: Arc::new(Semaphore::new(max_concurrent_executions)),
            plan_receiver: Some(plan_receiver),
            execution_results: result_sender,
            is_active: Arc::new(RwLock::new(false)),
            metrics: Arc::new(RwLock::new(ExecutionMetrics::default())),
        };

        // Return the plan_sender wrapped in a way that can be used by planner
        let (plan_tx, plan_rx) = mpsc::unbounded_channel();
        (executor, plan_rx, result_receiver)
    }

    /// Start the execution loop
    pub async fn start(&mut self) -> StdResult<(), AgentError> {
        {
            let mut is_active = self.is_active.write().await;
            if *is_active {
                return Ok(());
            }
            *is_active = true;
        }

        let mut plan_receiver = self.plan_receiver.take()
            .ok_or_else(|| AgentError::Configuration("Executor already started".to_string()))?;

        info!("Starting executor");

        while *self.is_active.read().await {
            tokio::select! {
                Some(plan) = plan_receiver.recv() => {
                    // Clone necessary data for async execution
                    let executor_clone = ExecutorHandle {
                        icm_client: Arc::clone(&self.icm_client),
                        http_client: self.http_client.clone(),
                        execution_semaphore: Arc::clone(&self.execution_semaphore),
                        result_sender: self.execution_results.clone(),
                        metrics: Arc::clone(&self.metrics),
                    };

                    // Execute plan concurrently
                    tokio::spawn(async move {
                        executor_clone.execute_plan(plan).await;
                    });
                }
            }
        }

        info!("Executor stopped");
        Ok(())
    }

    /// Stop the executor
    pub async fn stop(&self) {
        let mut is_active = self.is_active.write().await;
        *is_active = false;
        info!("Executor stop signal sent");
    }

    /// Get execution metrics
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        (*self.metrics.read().await).clone()
    }

    /// Get executor statistics
    pub async fn get_stats(&self) -> ExecutorStats {
        let metrics = self.metrics.read().await;
        let available_permits = self.execution_semaphore.available_permits();
        
        ExecutorStats {
            is_active: *self.is_active.read().await,
            available_permits,
            total_executions: metrics.total_executions,
            success_rate: if metrics.total_executions > 0 {
                metrics.successful_executions as f64 / metrics.total_executions as f64
            } else {
                0.0
            },
            avg_execution_time_ms: metrics.avg_execution_time_ms,
        }
    }
}

/// Helper struct for executing plans concurrently
struct ExecutorHandle {
    icm_client: Arc<IcmProgramInstance>,
    http_client: Client,
    execution_semaphore: Arc<Semaphore>,
    result_sender: mpsc::UnboundedSender<ExecutionResult>,
    metrics: Arc<RwLock<ExecutionMetrics>>,
}

impl ExecutorHandle {
    /// Execute a single trading plan
    async fn execute_plan(&self, plan: TradingPlan) {
        let start_time = Instant::now();
        let plan_id = plan.id;

        // Acquire execution permit
        let permit = match self.execution_semaphore.acquire().await {
            Ok(permit) => permit,
            Err(e) => {
                error!("Failed to acquire execution permit: {}", e);
                self.send_failure_result(plan_id, "Failed to acquire execution permit".to_string(), start_time).await;
                return;
            }
        };

        info!("Executing plan {} for strategy {:?}", plan_id, plan.strategy_type);

        // Check if plan is still valid (not expired)
        if Utc::now() > plan.expires_at {
            warn!("Plan {} expired, skipping execution", plan_id);
            self.send_failure_result(plan_id, "Plan expired".to_string(), start_time).await;
            drop(permit);
            return;
        }

        let result = match self.execute_swap(&plan).await {
            Ok(tx_response) => ExecutionResult {
                plan_id,
                success: true,
                transaction_signature: Some(tx_response.transaction),
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                actual_slippage_bps: None, // Would be calculated from actual execution
                error_message: None,
                gas_used: Some(5000), // Placeholder - would get from transaction receipt
                timestamp: Utc::now(),
            },
            Err(e) => ExecutionResult {
                plan_id,
                success: false,
                transaction_signature: None,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                actual_slippage_bps: None,
                error_message: Some(e.to_string()),
                gas_used: None,
                timestamp: Utc::now(),
            },
        };

        // Update metrics
        self.update_metrics(&result).await;

        // Send result
        if let Err(e) = self.result_sender.send(result) {
            error!("Failed to send execution result: {}", e);
        }

        drop(permit);
    }

    /// Execute the swap transaction
    async fn execute_swap(&self, plan: &TradingPlan) -> StdResult<UnsignedTransactionResponse, AgentError> {
        // Directly call the agent_swap_tokens_transaction method from IcmProgramInstance
        // You may need to load the keypair and other arguments as needed
        let keypair = read_keypair_file("/path/to/your/keypair.json")
            .map_err(|e| AgentError::Configuration(format!("Failed to load keypair: {}", e)))?;

        let swap_request = SwapTokensRequest {
            bucket: plan.bucket_pubkey.to_string(),
            input_mint: plan.input_mint.to_string(),
            output_mint: plan.output_mint.to_string(),
            route_plan: plan.route_plan.clone(),
            in_amount: plan.input_amount,
            quoted_out_amount: plan.min_output_amount,
            slippage_bps: plan.max_slippage_bps,
            platform_fee_bps: 50, // Could be configured
        };

        // Fetch bucket_name from the database using plan.bucket_pubkey
        // This is a stub. Replace with actual DB fetch logic as needed.
    let bucket_name = Self::fetch_bucket_name_by_pubkey(plan.bucket_pubkey)
            .await
            .map_err(|e| AgentError::Configuration(format!("Failed to fetch bucket name: {}", e)))?;

        let input_mint = plan.input_mint;
        let output_mint = plan.output_mint;
        let jupiter_program = Pubkey::from_str(&std::env::var("JUPITER_PROGRAM_PUBKEY").expect("JUPITER_PROGRAM_PUBKEY env var required")).expect("Invalid JUPITER_PROGRAM_PUBKEY");
        let platform_fee_account = Pubkey::from_str(&std::env::var("PLATFORM_FEE_ACCOUNT").expect("PLATFORM_FEE_ACCOUNT env var required")).expect("Invalid PLATFORM_FEE_ACCOUNT");
        let token_2022_program = TOKEN_2022_PROGRAM_ID;
        let input_mint_program = Pubkey::default();
        let output_mint_program = token_2022_program;

        let tx_response = self.icm_client.agent_swap_tokens_transaction(
            swap_request,
            keypair,
            &bucket_name,
            input_mint,
            output_mint,
            jupiter_program,
            token_2022_program,
            platform_fee_account,
            input_mint_program,
            output_mint_program,
        ).await.map_err(|e| AgentError::TransactionFailed(format!("ICM swap failed: {}", e)))?;

        Ok(tx_response)
    }

// Stub for fetching bucket_name from DB by pubkey
pub async fn fetch_bucket_name_by_pubkey(_bucket_pubkey: Pubkey) -> StdResult<String, AgentError> {
    // TODO: Implement actual DB lookup
    Ok("bucket_name_from_db".to_string())
}

    /// Get swap instructions from Jupiter API
    async fn get_jupiter_swap_instructions(&self, plan: &TradingPlan) -> StdResult<Value, AgentError> {
        let swap_request = json!({
            "quoteResponse": {
                "inputMint": plan.input_mint.to_string(),
                "outputMint": plan.output_mint.to_string(),
                "inAmount": plan.input_amount.to_string(),
                "outAmount": plan.min_output_amount.to_string(),
                "routePlan": self.decode_route_plan(&plan.route_plan)?,
            },
            "userPublicKey": plan.bucket_pubkey.to_string(),
            "wrapAndUnwrapSol": true,
            "feeAccount": plan.bucket_pubkey.to_string(),
            "computeUnitPriceMicroLamports": plan.priority_fee,
            "prioritizationFeeLamports": plan.priority_fee,
        });

        let response_result = timeout(
            Duration::from_millis(10000), // 10 second timeout
            self.http_client
                .post(&format!("{}/swap-instructions", JUPITER_SWAP_API))
                .json(&swap_request)
                .send()
        ).await;

        let response = match response_result {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => return Err(AgentError::Network(e)),
            Err(_) => return Err(AgentError::TransactionFailed("Jupiter API timeout".to_string())),
        };

        if !response.status().is_success() {
            let err_msg = format!("Swap instructions failed: HTTP {}", response.status());
            return Err(AgentError::JupiterApi(err_msg));
        }

        let instructions: Value = match response.json().await {
            Ok(val) => val,
            Err(e) => return Err(AgentError::Network(e)),
        };
        Ok(instructions)
    }

    /// Decode route plan from binary format
    fn decode_route_plan(&self, encoded: &[u8]) -> StdResult<Value, AgentError> {
        // Decode the route plan that was encoded by the strategy
        let route_plan: Vec<crate::agent::types::RoutePlan> = bincode::deserialize(encoded)
            .map_err(|e| AgentError::Configuration(format!("Failed to decode route plan: {}", e)))?;

        // Convert to Jupiter API format
        let jupiter_format = route_plan.iter().map(|rp| {
            json!({
                "swapInfo": {
                    "ammKey": rp.swap_info.amm_key,
                    "label": rp.swap_info.label,
                    "inputMint": rp.swap_info.input_mint,
                    "outputMint": rp.swap_info.output_mint,
                    "inAmount": rp.swap_info.in_amount,
                    "outAmount": rp.swap_info.out_amount,
                    "feeAmount": rp.swap_info.fee_amount,
                    "feeMint": rp.swap_info.fee_mint,
                },
                "percent": rp.percent
            })
        }).collect::<Vec<_>>();

        Ok(json!(jupiter_format))
    }

    /// Update execution metrics
    async fn update_metrics(&self, result: &ExecutionResult) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_executions += 1;
        if result.success {
            metrics.successful_executions += 1;
        } else {
            metrics.failed_executions += 1;
        }

        // Update average execution time
        if metrics.total_executions == 1 {
            metrics.avg_execution_time_ms = result.execution_time_ms;
        } else {
            metrics.avg_execution_time_ms = (
                (metrics.avg_execution_time_ms * (metrics.total_executions - 1)) + result.execution_time_ms
            ) / metrics.total_executions;
        }

        if let Some(gas_used) = result.gas_used {
            metrics.total_gas_used += gas_used;
        }

        metrics.last_execution = Some(result.timestamp);

        debug!("Updated metrics: total={}, success={}, avg_time={}ms",
               metrics.total_executions,
               metrics.successful_executions,
               metrics.avg_execution_time_ms);
    }

    /// Send failure result
    async fn send_failure_result(&self, plan_id: uuid::Uuid, error: String, start_time: Instant) {
        let result = ExecutionResult {
            plan_id,
            success: false,
            transaction_signature: None,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            actual_slippage_bps: None,
            error_message: Some(error),
            gas_used: None,
            timestamp: Utc::now(),
        };

        self.update_metrics(&result).await;

        if let Err(e) = self.result_sender.send(result) {
            error!("Failed to send failure result: {}", e);
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ExecutorStats {
    pub is_active: bool,
    pub available_permits: usize,
    pub total_executions: u64,
    pub success_rate: f64,
    pub avg_execution_time_ms: u64,
}

/// Retry configuration for failed executions
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u8,
    pub initial_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 10000,
        }
    }
}

/// Transaction status for monitoring
#[derive(Debug, Clone)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Expired,
}

/// Enhanced execution result with more details
#[derive(Debug, Clone)]
pub struct DetailedExecutionResult {
    pub basic: ExecutionResult,
    pub transaction_status: Option<TransactionStatus>,
    pub block_slot: Option<u64>,
    pub confirmation_count: Option<u8>,
    pub retry_count: u8,
    pub final_output_amount: Option<u64>,
    pub fees_paid: Option<u64>,
}
