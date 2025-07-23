use anchor_client::{ Client, Cluster, Program };
use anchor_lang::prelude::*;
use anyhow::{ anyhow, Result };
use solana_sdk::{
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
    instruction::{AccountMeta, Instruction},
};
use spl_token::ID as TOKEN_PROGRAM_ID;
use spl_associated_token_account::ID as ASSOCIATED_TOKEN_PROGRAM_ID;
use std::{ rc::Rc, str::FromStr };
use serde::{ Deserialize, Serialize };

// Program ID from the IDL
pub const ICM_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("ChzF1fiEZ8Uo2q62dg2f8MJSQW2oDxw4PJcbWiC1pxQe");

/// ICM Program client instance for handling transactions
pub struct IcmProgramInstance {
    cluster: Cluster,
    payer_pubkey: Pubkey,
}

/// Request structures for API calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBucketRequest {
    pub name: String,
    pub token_mints: Vec<String>,
    pub contribution_window_days: u32,
    pub trading_window_days: u32,
    pub creator_fee_percent: u16,
    pub creator_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributeToBucketRequest {
    pub bucket_name: String,
    pub token_mint: String,
    pub amount: u64,
    pub contributor_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartTradingRequest {
    pub bucket_name: String,
    pub creator_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapTokensRequest {
    pub creator_pubkey: String,
    pub bucket_pubkey: String,
    pub input_mint: String,
    pub output_mint: String,
    pub route_plan: Vec<u8>,
    pub in_amount: u64,
    pub quoted_out_amount: u64,
    pub slippage_bps: u16,
    pub platform_fee_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimRewardsRequest {
    pub bucket_name: String,
    pub token_mint: String,
    pub contributor_pubkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseBucketRequest {
    pub bucket_name: String,
    pub creator_pubkey: String,
}

/// Response structure containing unsigned transaction for frontend signing
#[derive(Debug, Serialize)]
pub struct UnsignedTransactionResponse {
    pub transaction: String, // Base64 encoded transaction
    pub message: String,
}

/// Query parameters for getting bucket information
#[derive(Debug, Deserialize)]
pub struct GetBucketQuery {
    pub name: String,
    pub creator: String,
}

/// Bucket information response
#[derive(Debug, Serialize)]
pub struct BucketInfo {
    pub name: String,
    pub creator: String,
    pub status: String,
    pub contribution_deadline: i64,
    pub trading_deadline: i64,
}

impl IcmProgramInstance {
    /// Create a new instance of the ICM program client
    pub fn new(cluster: Cluster, payer: Keypair) -> Result<Self> {
        Ok(Self {
            cluster,
            payer_pubkey: payer.pubkey(),
        })
    }

    /// Helper method to create placeholder unsigned transaction response
    fn create_placeholder_transaction(&self, message: String) -> UnsignedTransactionResponse {
        // Create a dummy transaction for now - in production this would be a real transaction
        let dummy_tx_data = vec![0u8; 64]; // Placeholder transaction data
        let encoded_tx = base64::encode(&dummy_tx_data);
        
        UnsignedTransactionResponse {
            transaction: encoded_tx,
            message,
        }
    }

    /// Create bucket transaction for frontend signing
    pub async fn create_bucket_transaction(
        &self,
        request: CreateBucketRequest
    ) -> Result<UnsignedTransactionResponse> {
        // Validate input
        let _creator = Pubkey::from_str(&request.creator_pubkey)?;
        let _token_mints: Result<Vec<Pubkey>, _> = request.token_mints
            .iter()
            .map(|mint| Pubkey::from_str(mint))
            .collect();
        let _token_mints = _token_mints?;

        // For now, return a placeholder transaction
        // TODO: Implement actual transaction building with proper IDL integration
        Ok(self.create_placeholder_transaction(
            format!("Create bucket '{}'", request.name)
        ))
    }

    /// Contribute to bucket transaction for frontend signing
    pub async fn contribute_to_bucket_transaction(
        &self,
        request: ContributeToBucketRequest
    ) -> Result<UnsignedTransactionResponse> {
        // Validate input
        let _contributor = Pubkey::from_str(&request.contributor_pubkey)?;
        let _token_mint = Pubkey::from_str(&request.token_mint)?;

        // For now, return a placeholder transaction
        Ok(self.create_placeholder_transaction(
            format!("Contribute {} tokens to bucket '{}'", request.amount, request.bucket_name)
        ))
    }

    /// Start trading transaction for frontend signing
    pub async fn start_trading_transaction(
        &self,
        request: StartTradingRequest
    ) -> Result<UnsignedTransactionResponse> {
        // Validate input
        let _creator = Pubkey::from_str(&request.creator_pubkey)?;

        Ok(self.create_placeholder_transaction(
            format!("Start trading for bucket '{}'", request.bucket_name)
        ))
    }

    /// Swap tokens transaction for frontend signing
    pub async fn swap_tokens_transaction(
        &self,
        request: SwapTokensRequest
    ) -> Result<UnsignedTransactionResponse> {
        // Validate input
        let _creator = Pubkey::from_str(&request.creator_pubkey)?;
        let _bucket = Pubkey::from_str(&request.bucket_pubkey)?;
        let _input_mint = Pubkey::from_str(&request.input_mint)?;
        let _output_mint = Pubkey::from_str(&request.output_mint)?;

        Ok(self.create_placeholder_transaction(
            format!("Swap {} tokens in bucket", request.in_amount)
        ))
    }

    /// Claim rewards transaction for frontend signing
    pub async fn claim_rewards_transaction(
        &self,
        request: ClaimRewardsRequest
    ) -> Result<UnsignedTransactionResponse> {
        // Validate input
        let _contributor = Pubkey::from_str(&request.contributor_pubkey)?;
        let _token_mint = Pubkey::from_str(&request.token_mint)?;

        Ok(self.create_placeholder_transaction(
            format!("Claim rewards from bucket '{}'", request.bucket_name)
        ))
    }

    /// Close bucket transaction for frontend signing
    pub async fn close_bucket_transaction(
        &self,
        request: CloseBucketRequest
    ) -> Result<UnsignedTransactionResponse> {
        // Validate input
        let _creator = Pubkey::from_str(&request.creator_pubkey)?;

        Ok(self.create_placeholder_transaction(
            format!("Close bucket '{}'", request.bucket_name)
        ))
    }
}
