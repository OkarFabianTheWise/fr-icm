//! Swap Engine Service
//! 
//! Handles currency swaps, USDC ↔ ETF conversions, and routing logic.
//! Core revenue-generating service through swap fees.

use anyhow::{Context, Result};
use std::str::FromStr;
use async_trait::async_trait;
use bigdecimal::{BigDecimal, Zero};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use deadpool_postgres::Pool;
use std::collections::HashMap;
use uuid::Uuid;
use tower_async::Service;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    transaction::VersionedTransaction,
};
use solana_sdk::system_instruction;
use solana_sdk::transaction::Transaction;
use spl_associated_token_account::{get_associated_token_address, create_associated_token_account};
use crate::database::models::{Swap, CreateSwapRequest, FromRow};
use bs58;

// Constants
const RPC_URL: &str = "https://hidden-broken-yard.solana-mainnet.quiknode.pro/7fef0c379b4a84c33cf93ab6d9ada7a5916eba9b";
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const JUPITER_QUOTE_API: &str = "https://quote-api.jup.ag/v6/quote";
const JUPITER_SWAP_API: &str = "https://quote-api.jup.ag/v6/swap";

/// Jupiter API quote response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterQuote {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "swapUsdValue")]
    pub swap_usd_value: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: Option<String>,
    #[serde(rename = "swapMode")]
    pub swap_mode: Option<String>,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: Option<u16>,
    #[serde(rename = "platformFee")]
    pub platform_fee: Option<serde_json::Value>,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: Option<String>,
    #[serde(rename = "routePlan")]
    pub route_plan: Option<serde_json::Value>,
    #[serde(rename = "contextSlot")]
    pub context_slot: Option<u64>,
    #[serde(rename = "timeTaken")]
    pub time_taken: Option<f64>,
}

/// Jupiter API swap response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterSwapResponse {
    #[serde(rename = "swapTransaction")]
    pub swap_transaction: String,
}

/// Jupiter swap request payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JupiterSwapRequest {
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "quoteResponse")]
    pub quote_response: JupiterQuote,
    #[serde(rename = "destinationTokenAccount")]
    pub destination_token_account: Option<String>,
    #[serde(rename = "computeUnitPriceMicroLamports")]
    pub compute_unit_price_micro_lamports: Option<u64>,
}

/// Solana swap service using tower-async
pub struct SolanaSwapService {
    rpc_client: RpcClient,
    keypair: Keypair,
    trader_pubkey: Option<Pubkey>,
}

impl Clone for SolanaSwapService {
    fn clone(&self) -> Self {
        Self {
            rpc_client: RpcClient::new_with_commitment(
                RPC_URL.to_string(),
                CommitmentConfig::confirmed(),
            ),
            keypair: Keypair::try_from(&self.keypair.to_bytes()[..]).unwrap(),
            trader_pubkey: self.trader_pubkey,
        }
    }
}

impl SolanaSwapService {
    /// Create a new Solana swap service
    pub fn new(private_key: &str, trader_pubkey: Option<String>) -> Result<Self> {
        let keypair_bytes = bs58::decode(private_key)
            .into_vec()
            .context("Failed to decode private key")?;
        
        let keypair = Keypair::try_from(&keypair_bytes[..])
            .context("Failed to create keypair from bytes")?;
        
        let trader_pubkey = trader_pubkey
            .map(|pk| Pubkey::from_str(&pk))
            .transpose()
            .context("Invalid trader public key")?;
        
        let rpc_client = RpcClient::new_with_commitment(
            RPC_URL.to_string(),
            CommitmentConfig::confirmed(),
        );
        
        Ok(Self {
            rpc_client,
            keypair,
            trader_pubkey,
        })
    }
    
    /// Get the trader's public key (defaults to service keypair if not specified)
    pub fn get_trader_pubkey(&self) -> Pubkey {
        self.trader_pubkey.unwrap_or(self.keypair.pubkey())
    }
    
    /// Check if a token is Token-2022
    pub async fn is_token_2022(&self, mint: &str) -> Result<bool> {
        let mint_pubkey = Pubkey::from_str(mint)
            .context("Invalid mint address")?;
        
        let account_info = self.rpc_client
            .get_account(&mint_pubkey)
            .await
            .context("Failed to get mint account info")?;
        
        let token_2022_program = Pubkey::from_str(TOKEN_2022_PROGRAM_ID)
            .context("Invalid Token-2022 program ID")?;
        
        Ok(account_info.owner == token_2022_program)
    }
    
    /// Get or create associated token account for destination
    pub async fn get_or_create_dest_token_account(&self, output_mint: &str) -> Result<Option<Pubkey>> {
        let mint = Pubkey::from_str(output_mint)
            .context("Invalid output mint")?;
        
        let trader = self.get_trader_pubkey();
        
        // Determine if this is Token-2022
        let is_token_2022 = self.is_token_2022(output_mint).await?;
        let token_program_id = if is_token_2022 {
            Pubkey::from_str(TOKEN_2022_PROGRAM_ID)?
        } else {
            spl_token::ID
        };
        
        let associated_token_address = get_associated_token_address(&trader, &mint);
        
        // Check if account exists
        match self.rpc_client.get_account(&associated_token_address).await {
            Ok(_) => Ok(Some(associated_token_address)),
            Err(_) => {
                // Create associated token account
                let instruction = create_associated_token_account(
                    &self.keypair.pubkey(),
                    &trader,
                    &mint,
                );
                
                let transaction = Transaction::new_signed_with_payer(
                    &[instruction],
                    Some(&self.keypair.pubkey()),
                    &[&self.keypair],
                    self.rpc_client.get_latest_blockhash().await?,
                );
                
                let _signature = self.rpc_client
                    .send_and_confirm_transaction(&transaction)
                    .await
                    .context("Failed to create associated token account")?;
                
                Ok(Some(associated_token_address))
            }
        }
    }
    
    /// Fetch quote from Jupiter
    pub async fn fetch_quote(&self, output_mint: &str, swap_amount: u64) -> Result<JupiterQuote> {
        let client = reqwest::Client::new();
        
        let params = [
            ("inputMint", USDC_MINT),
            ("outputMint", output_mint),
            ("slippageBps", "1000"), // 10% slippage
            ("amount", &swap_amount.to_string()),
        ];
        
        let response = client
            .get(JUPITER_QUOTE_API)
            .query(&params)
            .send()
            .await
            .context("Failed to send quote request")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Jupiter quote API returned error: {}", response.status());
        }
        
        let quote: JupiterQuote = response
            .json()
            .await
            .context("Failed to parse quote response")?;
        
        Ok(quote)
    }
    
    /// Transfer SOL to trader's wallet
    pub async fn transfer_sol_to_trader(&self, amount: u64) -> Result<Signature> {
        let trader = self.get_trader_pubkey();
        
        let instruction = system_instruction::transfer(
            &self.keypair.pubkey(),
            &trader,
            amount,
        );
        
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&self.keypair.pubkey()),
            &[&self.keypair],
            self.rpc_client.get_latest_blockhash().await?,
        );
        
        let signature = self.rpc_client
            .send_and_confirm_transaction(&transaction)
            .await
            .context("Failed to transfer SOL to trader")?;
        
        Ok(signature)
    }
    
    /// Perform direct USDC transfer (USDC -> USDC)
    pub async fn transfer_usdc_direct(&self, amount: u64) -> Result<Signature> {
        let usdc_mint = Pubkey::from_str(USDC_MINT)?;
        let trader = self.get_trader_pubkey();
        
        let source_ata = get_associated_token_address(&self.keypair.pubkey(), &usdc_mint);
        let dest_ata = get_associated_token_address(&trader, &usdc_mint);
        
        let mut instructions = vec![];
        
        // Check if destination account exists, create if not
        if self.rpc_client.get_account(&dest_ata).await.is_err() {
            instructions.push(create_associated_token_account(
                &self.keypair.pubkey(),
                &trader,
                &usdc_mint,
            ));
        }
        
        // Add transfer instruction
        instructions.push(spl_token::instruction::transfer(
            &spl_token::ID,
            &source_ata,
            &dest_ata,
            &self.keypair.pubkey(),
            &[],
            amount,
        )?);
        
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&self.keypair.pubkey()),
            &[&self.keypair],
            self.rpc_client.get_latest_blockhash().await?,
        );
        
        let signature = self.rpc_client
            .send_and_confirm_transaction(&transaction)
            .await
            .context("Failed to transfer USDC")?;
        
        Ok(signature)
    }
    
    /// Execute swap through Jupiter
    pub async fn perform_swap(&self, output_mint: &str, swap_amount: u64) -> Result<String> {
        // Handle direct USDC transfer
        if output_mint == USDC_MINT {
            let signature = self.transfer_usdc_direct(swap_amount).await?;
            return Ok(signature.to_string());
        }
        
        // Get quote from Jupiter
        let quote = self.fetch_quote(output_mint, swap_amount).await?;
        
        // Handle USDC -> SOL swap
        if output_mint == SOL_MINT {
            let swap_request = JupiterSwapRequest {
                user_public_key: self.keypair.pubkey().to_string(),
                quote_response: quote.clone(),
                destination_token_account: None,
                compute_unit_price_micro_lamports: Some(30_000_000),
            };
            
            let client = reqwest::Client::new();
            let response = client
                .post(JUPITER_SWAP_API)
                .json(&swap_request)
                .send()
                .await
                .context("Failed to send swap request")?;

            let swap_response: JupiterSwapResponse = response
                .json()
                .await
                .context("Failed to parse swap response")?;
            
            // Deserialize and partially sign Jupiter swap transaction
            let transaction_bytes = base64::decode(&swap_response.swap_transaction)
                .context("Failed to decode swap transaction")?;

            let mut versioned_tx: VersionedTransaction = bincode::deserialize(&transaction_bytes)
                .context("Failed to deserialize versioned transaction")?;

            // Find the signer index in the message's account keys
            let message_keys = versioned_tx.message.static_account_keys();
            let idx = message_keys
                .iter()
                .position(|key| key == &self.keypair.pubkey())
                .context("Keypair not in message account keys")?;

            // Sign the serialized message and assign at correct index
            let msg_data = versioned_tx.message.serialize();
            let sig = self.keypair.sign_message(&msg_data);
            versioned_tx.signatures[idx] = sig;

            // Send transaction
            let signature = self.rpc_client
                .send_and_confirm_transaction(&versioned_tx)
                .await
                .context("Failed to send swap transaction")?;

            
            // Transfer swapped SOL to trader
            let out_amount: u64 = quote.out_amount.parse()
                .context("Failed to parse output amount")?;
            
            let transfer_signature = self.transfer_sol_to_trader(out_amount).await?;
            return Ok(transfer_signature.to_string());
        }
        
        // Handle USDC -> Token swap
        let destination_account = self.get_or_create_dest_token_account(output_mint).await?;
        
        let swap_request = JupiterSwapRequest {
            user_public_key: self.keypair.pubkey().to_string(),
            quote_response: quote,
            destination_token_account: destination_account.map(|pk| pk.to_string()),
            compute_unit_price_micro_lamports: Some(30_000_000),
        };
        
        let client = reqwest::Client::new();
        let response = client
            .post(JUPITER_SWAP_API)
            .json(&swap_request)
            .send()
            .await
            .context("Failed to send swap request")?;
        
        let swap_response: JupiterSwapResponse = response
            .json()
            .await
            .context("Failed to parse swap response")?;
        
        // Deserialize and partially sign Jupiter swap transaction
        let transaction_bytes = base64::decode(&swap_response.swap_transaction)
            .context("Failed to decode swap transaction")?;

        let mut versioned_tx: VersionedTransaction = bincode::deserialize(&transaction_bytes)
            .context("Failed to deserialize versioned transaction")?;

        // Find the signer index in the message's account keys
        let message_keys = versioned_tx.message.static_account_keys();
        let idx = message_keys
            .iter()
            .position(|key| key == &self.keypair.pubkey())
            .context("Keypair not in message account keys")?;

        // Sign the serialized message and assign at correct index
        let msg_data = versioned_tx.message.serialize();
        let sig = self.keypair.sign_message(&msg_data);
        versioned_tx.signatures[idx] = sig;

        // Send transaction
        let signature = self.rpc_client
            .send_and_confirm_transaction(&versioned_tx)
            .await
            .context("Failed to send swap transaction")?;
        
        Ok(signature.to_string())
    }
}

/// Tower async service implementation for Solana swaps
#[derive(Clone)]
pub struct SwapRequest {
    pub swap_amount: f64,
    pub output_mint: String,
    pub trader: Option<String>,
    pub private_key: String,
}

#[derive(Clone)]
pub struct SwapResponse {
    pub transaction_id: String,
}

impl Service<SwapRequest> for SolanaSwapService {
    type Response = SwapResponse;
    type Error = anyhow::Error;
    
    async fn call(&self, req: SwapRequest) -> Result<Self::Response, Self::Error> {
        let lamports = (req.swap_amount * 1_000_000.0) as u64; // USDC uses 6 decimals
        let transaction_id = self.perform_swap(&req.output_mint, lamports).await?;
        
        Ok(SwapResponse { transaction_id })
    }
}

/// Initialize a buy swap (exported function matching TypeScript interface)
pub async fn initiate_buy_swap(
    swap_amount: f64,
    output_mint: String,
    trader: Option<String>,
    private_key: String,
) -> Result<String> {
    let solana_swap = SolanaSwapService::new(&private_key, trader)?;
    let lamports = (swap_amount * 1_000_000.0) as u64; // USDC uses 6 decimals
    
    match solana_swap.perform_swap(&output_mint, lamports).await {
        Ok(transaction_id) => Ok(transaction_id),
        Err(_) => Ok("Transaction failed! Reach out to support".to_string()),
    }
}
