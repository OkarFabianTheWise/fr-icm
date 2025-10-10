// Add this constant for vault seed if not already present
use anchor_client::{Client, Cluster};
use anchor_lang::prelude::*;
use std::sync::Arc;
use std::str::FromStr;
use anyhow::{anyhow, Result};
use base64;
use bincode;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    sysvar,
};
use solana_sdk::transaction::Transaction;
use spl_associated_token_account::get_associated_token_address;

declare_program!(icm_program);
pub const ICM_PROGRAM_ID: Pubkey = icm_program::ID;
pub const VAULT_SEED: &[u8] = b"vault";

use icm_program::client::args::{CreateBucket, ContributeToBucket, StartTrading, ClaimRewards, CloseBucket};
use icm_program::client::accounts::{CreateBucket as CreateBucketAccount, ContributeToBucket as ContributeToBucketAccount, StartTrading as StartTradingAccount, SwapTokens as SwapTokensAccount, ClaimRewards as ClaimRewardsAccount, CloseBucket as CloseBucketAccount, CreateProfile as CreateProfileAccount};
pub use crate::state_structs::{TradingPool, CreatorProfile, BucketAccount, BucketInfo};
use crate::state_structs::{CreateBucketRequest, ContributeToBucketRequest, StartTradingRequest, SwapTokensRequest, ClaimRewardsRequest, CloseBucketRequest, UnsignedTransactionResponse, GetCreatorProfileQuery, GetBucketQuery};

/// Format seconds into a human-readable time string
fn format_time_remaining(seconds: i64) -> String {
    if seconds <= 0 {
        return "Ended".to_string();
    }
    
    let days = seconds / (24 * 3600);
    let hours = (seconds % (24 * 3600)) / 3600;
    let minutes = (seconds % 3600) / 60;
    
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{}d", days));
    }
    if hours > 0 {
        parts.push(format!("{}h", hours));
    }
    if minutes > 0 || parts.is_empty() {
        parts.push(format!("{}m", minutes));
    }
    
    parts.join(" ")
}

/// Convert lamports to human-readable USDC amount (divide by 1e6)
fn lamports_to_usdc(lamports: u64) -> f64 {
    lamports as f64 / 1_000_000.0
}

/// Calculate time remaining for a pool based on its status and current time
fn calculate_time_remaining_for_bucket(
    status: &str,
    contribution_deadline: i64,
    trading_deadline: i64,
    trading_started_at: i64,
    current_time: i64
) -> Option<String> {
    match status {
        "Raising" => {
            if current_time < contribution_deadline {
                let time_left = contribution_deadline - current_time;
                Some(format_time_remaining(time_left))
            } else {
                Some("Ended".to_string())
            }
        },
        "Trading" => {
            if trading_started_at > 0 && current_time < trading_deadline {
                let time_left = trading_deadline - current_time;
                Some(format_time_remaining(time_left))
            } else {
                Some("Ended".to_string())
            }
        },
        _ => Some("Ended".to_string())
    }
}

#[derive(Debug, Clone)]
pub struct IcmProgramInstance {
    pub cluster: Cluster,
    pub payer_pubkey: Pubkey,
}

impl IcmProgramInstance {
    /// Create a new instance of the ICM program client
    pub fn new(cluster: Cluster, payer: Keypair) -> Result<Self> {
        println!("ICM Program ID: {}", ICM_PROGRAM_ID);
        Ok(Self {
            cluster,
            payer_pubkey: payer.pubkey(),
        })
    }
    
    /// Agent swap tokens transaction using Raydium
    pub async fn agent_swap_tokens_transaction(
        &self,
        request: SwapTokensRequest,
        keypair: Keypair,
        bucket_name: &str,
        input_mint: Pubkey,
        output_mint: Pubkey,
        raydium_amm_program: Pubkey,
        amm: Pubkey,
        amm_authority: Pubkey,
        pool_coin_token_account: Pubkey,
        pool_pc_token_account: Pubkey,
        user_authority: Pubkey,
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = keypair.pubkey();

        // Derive bucket PDA
        let (bucket_pda, _) = Pubkey::find_program_address(
            &[b"bucket", bucket_name.as_bytes(), creator.as_ref()],
            &ICM_PROGRAM_ID,
        );

        // Derive trade_record PDA
        let (trade_record_pda, _) = Pubkey::find_program_address(
            &[b"trade_record", bucket_pda.as_ref(), creator.as_ref()],
            &ICM_PROGRAM_ID,
        );

        // Derive vault input token account PDA
        let (vault_input_token_account, _) = Pubkey::find_program_address(
            &[VAULT_SEED, bucket_pda.as_ref(), input_mint.as_ref()],
            &ICM_PROGRAM_ID,
        );

        // Derive vault output token account PDA
        let (vault_output_token_account, _) = Pubkey::find_program_address(
            &[VAULT_SEED, bucket_pda.as_ref(), output_mint.as_ref()],
            &ICM_PROGRAM_ID,
        );

        let ixs = program
            .request()
            .accounts(SwapTokensAccount {
                trade_record: trade_record_pda,
                creator,
                bucket: bucket_pda,
                input_mint,
                system_program: system_program::id(),
                input_mint_program: spl_token::ID,
                output_mint,
                output_mint_program: spl_token::ID,
                vault_input_token_account,
                vault_output_token_account,
                raydium_amm_program,
                amm,
                amm_authority,
                pool_coin_token_account,
                pool_pc_token_account,
                user_source_token_account: vault_input_token_account,
                user_destination_token_account: vault_output_token_account,
                user_authority,
                token_program: spl_token::ID,
                rent: sysvar::rent::id(),
            })
            .args(icm_program::client::args::SwapTokens {
                in_amount: request.in_amount,
                quoted_out_amount: request.quoted_out_amount,
                slippage_bps: request.slippage_bps,
            })
            .instructions()?;

        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&creator),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;

        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: format!("Agent swap tokens for '{}'", creator.to_string()),
        })
    }

    /// Manual swap tokens transaction for frontend signing using Raydium
    pub async fn manual_swap_tokens_transaction(
        &self,
        request: SwapTokensRequest,
        keypair: Keypair,
        bucket_name: &str,
        input_mint: Pubkey,
        output_mint: Pubkey,
        raydium_amm_program: Pubkey,
        amm: Pubkey,
        amm_authority: Pubkey,
        pool_coin_token_account: Pubkey,
        pool_pc_token_account: Pubkey,
        user_authority: Pubkey,
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = keypair.pubkey();

        // Derive bucket PDA
        let (bucket_pda, _) = Pubkey::find_program_address(
            &[b"bucket", bucket_name.as_bytes(), creator.as_ref()],
            &ICM_PROGRAM_ID,
        );

        // Derive trade_record PDA
        let (trade_record_pda, _) = Pubkey::find_program_address(
            &[b"trade_record", bucket_pda.as_ref(), creator.as_ref()],
            &ICM_PROGRAM_ID,
        );

        // Derive vault input token account PDA
        let (vault_input_token_account, _) = Pubkey::find_program_address(
            &[VAULT_SEED, bucket_pda.as_ref(), input_mint.as_ref()],
            &ICM_PROGRAM_ID,
        );

        // Derive vault output token account PDA
        let (vault_output_token_account, _) = Pubkey::find_program_address(
            &[VAULT_SEED, bucket_pda.as_ref(), output_mint.as_ref()],
            &ICM_PROGRAM_ID,
        );

        let ixs = program
            .request()
            .accounts(SwapTokensAccount {
                trade_record: trade_record_pda,
                creator,
                bucket: bucket_pda,
                input_mint,
                system_program: system_program::id(),
                input_mint_program: spl_token::ID,
                output_mint,
                output_mint_program: spl_token::ID,
                vault_input_token_account,
                vault_output_token_account,
                raydium_amm_program,
                amm,
                amm_authority,
                pool_coin_token_account,
                pool_pc_token_account,
                user_source_token_account: vault_input_token_account,
                user_destination_token_account: vault_output_token_account,
                user_authority,
                token_program: spl_token::ID,
                rent: sysvar::rent::id(),
            })
            .args(icm_program::client::args::SwapTokens {
                in_amount: request.in_amount,
                quoted_out_amount: request.quoted_out_amount,
                slippage_bps: request.slippage_bps,
            })
            .instructions()?;

        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        
        // Create unsigned transaction for frontend signing
        let tx = Transaction::new_with_payer(
            &ixs,
            Some(&creator),
        );
        
        // Serialize transaction for frontend
        let serialized_tx = bincode::serialize(&tx)?;
        let base64_tx = base64::encode(serialized_tx);

        Ok(UnsignedTransactionResponse {
            transaction: base64_tx,
            message: format!("Swap tokens for '{}'", creator.to_string()),
        })
    }

    /// Create profile (creator profile) transaction
    pub async fn create_profile_transaction(
        &self,
        // no request parameter needed here
        keypair: Keypair
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = keypair.pubkey();
        let (creator_profile_pda, _) = Pubkey::find_program_address(&[b"creator_profile", creator.as_ref()], &ICM_PROGRAM_ID);

        let ixs = program
            .request()
            .accounts(CreateProfileAccount {
                creator_profile: creator_profile_pda,
                creator,
                system_program: system_program::id(),
            })
            .args(icm_program::client::args::CreateProfile {})
            .instructions()?;

        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&creator),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;

        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: format!("Create profile for '{}'", creator.to_string()),
        })
    }

    /// Create bucket transaction for frontend signing
    pub async fn create_bucket_transaction(
        &self,
        request: CreateBucketRequest,
        keypair: Keypair
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = keypair.pubkey();
        let token_mints: Vec<Pubkey> = request.token_mints.iter().map(|m| Pubkey::from_str(m).unwrap()).collect();

        // Derive PDAs for bucket, trading_pool, creator_profile
        let (bucket_pda, _) = Pubkey::find_program_address(&[b"bucket", request.name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        let (trading_pool_pda, _) = Pubkey::find_program_address(&[b"trading_pool", request.name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        // let (creator_profile_pda, _) = Pubkey::find_program_address(&[b"creator_profile", creator.as_ref()], &ICM_PROGRAM_ID);
        let usdc_mint = Pubkey::from_str("2RgRJx3z426TMCL84ZMXTRVCS5ee7iGVE4ogqcUAd3tg")?;
        // Derive contributor token account (ATA)
        let vault_token_account = get_associated_token_address(&bucket_pda, &usdc_mint);

        let ixs = program
            .request()
            .accounts(CreateBucketAccount {
                bucket: bucket_pda,
                trading_pool: trading_pool_pda,
                vault_token_account,
                usdc_mint: usdc_mint,
                creator,
                token_program: spl_token::ID,
                associated_token_program: spl_associated_token_account::ID,
                system_program: system_program::id(),
            })
            .args(CreateBucket {
                name: request.name.clone(),
                token_mints: token_mints.clone(),
                contribution_window_minutes: request.contribution_window_minutes, // Use minutes directly
                trading_window_minutes: request.trading_window_minutes,           // Use minutes directly
                creator_fee_percent: request.creator_fee_percent,
                target_amount: request.target_amount,
                min_contribution: request.min_contribution,
                max_contribution: request.max_contribution,
                management_fee: request.management_fee,
            })
            .instructions()?;

        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&creator),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;

        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: format!("Create bucket '{}'", request.name),
        })
    }

    /// Contribute to bucket transaction for frontend signing
    pub async fn contribute_to_bucket_transaction(
        &self,
        request: ContributeToBucketRequest,
        keypair: Keypair
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let contributor = keypair.pubkey();
        
        // log both contributor and keypair_for_sign.pubkey() for debugging
        tracing::error!("Contributor: {}", contributor);
        tracing::error!("Keypair for signing: {}", keypair_for_sign.pubkey());
        let creator = Pubkey::from_str(&request.creator_pubkey).map_err(|e| anyhow!(e))?;
        let usdc_mint: Pubkey = Pubkey::from_str("2RgRJx3z426TMCL84ZMXTRVCS5ee7iGVE4ogqcUAd3tg").expect("Invalid pubkey");

        // Derive bucket PDA: [b"bucket", bucket_name, creator]
        let (bucket_pda, _) = Pubkey::find_program_address(
            &[b"bucket", request.bucket_name.as_bytes(), creator.as_ref()],
            &ICM_PROGRAM_ID
        );

        // Derive contribution record PDA
        let (contribution_record_pda, _) = Pubkey::find_program_address(
            &[b"contribution", bucket_pda.as_ref(), contributor.as_ref(), usdc_mint.as_ref()],
            &ICM_PROGRAM_ID
        );

        // Derive pool contribution PDA
        let (pool_contribution_pda, _) = Pubkey::find_program_address(
            &[b"pool_contribution", bucket_pda.as_ref(), contributor.as_ref(), usdc_mint.as_ref()],
            &ICM_PROGRAM_ID
        );

        // Derive contributor token account (ATA)
        let contributor_token_account = get_associated_token_address(&contributor, &usdc_mint);

        // Derive vault token account PDA: [b"vault", bucket_pda, usdc_mint]
        let vault_token_account = get_associated_token_address(&bucket_pda, &usdc_mint);

        // Derive program_state PDA
        let (program_state_pda, _) = Pubkey::find_program_address(
            &[b"program_state"],
            &ICM_PROGRAM_ID,
        );

        // Derive fee_vault PDA (ATA for program_state and USDC mint)
        let fee_vault = get_associated_token_address(&program_state_pda, &usdc_mint);

        let ixs = program
            .request()
            .accounts(ContributeToBucketAccount{
                bucket: bucket_pda,
                contribution_record: contribution_record_pda,
                pool_contribution: pool_contribution_pda,
                contributor_token_account,
                vault_token_account,
                usdc_mint,
                program_state: program_state_pda,
                fee_vault,
                contributor,
                token_program: spl_token::ID,
                associated_token_program: spl_associated_token_account::ID,
                system_program: system_program::id(),
            })
            .args(ContributeToBucket {
                bucket_name: request.bucket_name.clone(),
                amount: request.amount,
            })
            .instructions()?;

        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&contributor),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;
        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: format!("Contribute to bucket '{}'", request.bucket_name),
        })
    }
        
    /// Claim rewards transaction for frontend signing
    pub async fn claim_rewards_transaction(
        &self,
        request: ClaimRewardsRequest,
        keypair: Keypair
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let contributor = keypair.pubkey();
        let (bucket_pda, _) = Pubkey::find_program_address(&[b"bucket", request.bucket_name.as_bytes(), contributor.as_ref()], &ICM_PROGRAM_ID);
        // TODO: Derive all required PDAs and accounts per IDL
        let contribution_record = Pubkey::default();
        let pool_contribution = Pubkey::default();
        let contributor_token_account = Pubkey::default();
        let vault_token_account = Pubkey::default();

        let accounts = vec![
            AccountMeta::new(bucket_pda, false),
            AccountMeta::new(contribution_record, false),
            AccountMeta::new(pool_contribution, true),
            AccountMeta::new(contributor_token_account, true),
            AccountMeta::new(vault_token_account, true),
            AccountMeta::new(contributor, true),
            AccountMeta::new_readonly(spl_token::ID, false),
        ];

        let ixs = program
            .request()
            .accounts(accounts)
            .args(ClaimRewards {
            })
            .instructions()?;
        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&contributor),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;

        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: format!("Claim rewards from bucket '{}'", request.bucket_name),
        })
    }

    /// Close bucket transaction for frontend signing
    pub async fn close_bucket_transaction(
        &self,
        request: CloseBucketRequest,
        keypair: Keypair
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = Pubkey::from_str(&request.creator_pubkey).map_err(|e| anyhow!(e))?;
        let (bucket_pda, _) = Pubkey::find_program_address(&[b"bucket", request.bucket_name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        let (trading_pool_pda, _) = Pubkey::find_program_address(&[b"trading_pool", request.bucket_name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        let (creator_profile_pda, _) = Pubkey::find_program_address(&[b"creator_profile", creator.as_ref()], &ICM_PROGRAM_ID);

        let accounts = vec![
            AccountMeta::new(bucket_pda, true),
            AccountMeta::new(trading_pool_pda, true),
            AccountMeta::new(creator_profile_pda, true),
            AccountMeta::new(creator, true),
        ];

        let ixs = program
            .request()
            .accounts(accounts)
            .args(CloseBucket {})
            .instructions()?;
        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&creator),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;

        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: format!("Close bucket '{}'", request.bucket_name),
        })
    }

    /// Fetch a TradingPool by PDA (public key)
    pub async fn fetch_trading_pool_by_pda(
        &self,
        request: GetBucketQuery,
        keypair: Keypair,
    ) -> Result<TradingPool> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;
        let creator = Pubkey::from_str(&request.creator_pubkey).map_err(|e| anyhow!(e))?;

        let (trading_pool_pda, _) = Pubkey::find_program_address(&[b"trading_pool", request.bucket_name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);

        let anchor_pool: icm_program::accounts::TradingPool = program.account(trading_pool_pda).await?;
        Ok(TradingPool {
            pool_id: trading_pool_pda.to_string(),
            pool_bump: anchor_pool.pool_bump,
            creator: anchor_pool.creator.to_string(),
            token_bucket: anchor_pool.token_bucket.iter().map(|pk| pk.to_string()).collect(),
            target_amount: anchor_pool.target_amount.to_string(),
            min_contribution: anchor_pool.min_contribution.to_string(),
            max_contribution: anchor_pool.max_contribution.to_string(),
            trading_duration: anchor_pool.trading_duration.to_string(),
            created_at: anchor_pool.created_at.to_string(),
            fundraising_deadline: anchor_pool.fundraising_deadline.to_string(),
            trading_start_time: anchor_pool.trading_start_time.map(|v| v.to_string()),
            trading_end_time: anchor_pool.trading_end_time.map(|v| v.to_string()),
            phase: format!("{:?}", anchor_pool.phase),
            management_fee: anchor_pool.management_fee,
            raised_amount: None,
            contribution_percent: None,
            strategy: None,
            time_remaining: None,
        })
    }

    /// Fetch a CreatorProfile by PDA (public key)
    pub async fn fetch_creator_profile_by_pda(
        &self, 
        request: GetCreatorProfileQuery,
        keypair: Keypair
    ) -> Result<CreatorProfile> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;
        let creator = Pubkey::from_str(&request.creator_pubkey).map_err(|e| anyhow!(e))?;
        let (pda, _) = Pubkey::find_program_address(&[b"creator_profile", creator.as_ref()], &ICM_PROGRAM_ID);

        let anchor_profile: icm_program::accounts::CreatorProfile = program.account(pda).await?;
        Ok(CreatorProfile {
            creator: anchor_profile.creator.to_string(),
            pools_created: anchor_profile.pools_created,
            successful_pools: anchor_profile.successful_pools,
            total_volume_managed: anchor_profile.total_volume_managed.to_string(),
            reputation_score: anchor_profile.reputation_score,
            created_at: anchor_profile.created_at.to_string(),
        })
    }

    /// Get all pools (buckets) - stub implementation
    pub async fn get_all_pools_by_pda(
        &self, 
        keypair: Keypair,
        db_pool: &deadpool_postgres::Pool
    ) -> Result<Vec<BucketInfo>> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        // Fetch all Bucket accounts
        let bucket_accounts = program.accounts::<icm_program::accounts::Bucket>(vec![]).await?;

        // Fetch all pool strategies from database for efficient lookup
        let strategies = crate::database::models::DatabaseTradingPool::fetch_all_pool_strategies(db_pool)
            .await
            .unwrap_or_else(|_| std::collections::HashMap::new());

        let buckets: Vec<BucketInfo> = bucket_accounts
            .into_iter()
            .map(|(pubkey, anchor_bucket)| {
                let creator = anchor_bucket.creator.to_string();
                let name = anchor_bucket.name.clone();
                let strategy_key = format!("{}_{}", creator, name);
                let strategy = strategies.get(&strategy_key).cloned();
                
                // Calculate time remaining
                let current_time = chrono::Utc::now().timestamp();
                let contribution_deadline = anchor_bucket.contribution_deadline;
                let trading_deadline = anchor_bucket.trading_deadline;
                let trading_started_at = anchor_bucket.trading_started_at;
                let status = format!("{:?}", anchor_bucket.status);
                
                let time_remaining = calculate_time_remaining_for_bucket(
                    &status,
                    contribution_deadline,
                    trading_deadline,
                    trading_started_at,
                    current_time
                );
                
                BucketInfo {
                    public_key: pubkey.to_string(),
                    account: BucketAccount {
                        creator,
                        name,
                        token_mints: anchor_bucket.token_mints.iter().map(|pk| pk.to_string()).collect(),
                        contribution_deadline: anchor_bucket.contribution_deadline.to_string(),
                        trading_deadline: anchor_bucket.trading_deadline.to_string(),
                        creator_fee_percent: anchor_bucket.creator_fee_percent,
                        status,
                        trading_started_at: anchor_bucket.trading_started_at.to_string(),
                        closed_at: anchor_bucket.closed_at.to_string(),
                        bump: anchor_bucket.bump,
                        creator_profile: anchor_bucket.creator_profile,
                        performance_fee: anchor_bucket.performance_fee,
                        raised_amount: lamports_to_usdc(anchor_bucket.raised_amount),
                        contributor_count: anchor_bucket.contributor_count,
                        strategy,
                        time_remaining,
                    },
                }
            })
            .collect();

        // for bucket in &buckets {
        //     println!("Bucket: {:?}", bucket);
        // }
        Ok(buckets)
    }

    /// Start trading transaction for frontend signing
    pub async fn start_trading_transaction(
        &self,
        request: StartTradingRequest,
        keypair: Keypair
    ) -> Result<UnsignedTransactionResponse> {
        tracing::info!("[start_trading_transaction] Starting transaction creation");
        tracing::info!("[start_trading_transaction] Request: bucket_name={}, creator_pubkey={}, strategy={}", 
            request.bucket_name, request.creator_pubkey, request.strategy);
        tracing::info!("[start_trading_transaction] Token bucket: {:?}", request.token_bucket);
        tracing::info!("[start_trading_transaction] Total amount: {}, management_fee: {}", 
            request.total_amount_available_to_trade, request.management_fee);
        
        let cluster = self.cluster.clone();
        tracing::debug!("[start_trading_transaction] Using cluster: {:?}", cluster);
        
        let keypair_for_client = keypair.insecure_clone();
        tracing::debug!("[start_trading_transaction] Creating client with keypair: {}", keypair_for_client.pubkey());
        
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        tracing::debug!("[start_trading_transaction] Client created successfully");
        
        let program = client.program(ICM_PROGRAM_ID)?;
        tracing::debug!("[start_trading_transaction] Program client created for ID: {}", ICM_PROGRAM_ID);

        let creator = Pubkey::from_str(&request.creator_pubkey).map_err(|e| anyhow!(e))?;
        tracing::info!("[start_trading_transaction] Creator pubkey parsed: {}", creator);
        
        let (bucket_pda, bucket_bump) = Pubkey::find_program_address(
            &[b"bucket", request.bucket_name.as_bytes(), creator.as_ref()], 
            &ICM_PROGRAM_ID
        );
        tracing::info!("[start_trading_transaction] Bucket PDA derived: {} (bump: {})", bucket_pda, bucket_bump);
        
        let (trading_pool_pda, trading_pool_bump) = Pubkey::find_program_address(
            &[b"trading_pool", request.bucket_name.as_bytes(), creator.as_ref()], 
            &ICM_PROGRAM_ID
        );
        tracing::info!("[start_trading_transaction] Trading pool PDA derived: {} (bump: {})", trading_pool_pda, trading_pool_bump);

        // Solend devnet accounts for USDC reserve
        tracing::info!("[start_trading_transaction] Setting up Solend accounts");
        let solend_program = Pubkey::from_str("ALend7Ketfx5bxh6ghsCDXAoDrhvEmsXT3cynB6aPLgx").unwrap();
        tracing::debug!("[start_trading_transaction] Solend program: {}", solend_program);
        
        let reserve = Pubkey::from_str("FNNkz4RCQezSSS71rW2tvqZH1LCkTzaiG7Nd1LeA5x5y").unwrap();
        tracing::debug!("[start_trading_transaction] Reserve: {}", reserve);
        
        let lending_market = Pubkey::from_str("GvjoVKNjBvQcFaSKUW1gTE7DxhSpjHbE69umVR5nPuQp").unwrap();
        tracing::debug!("[start_trading_transaction] Lending market: {}", lending_market);
        
        let pyth_oracle = Pubkey::from_str("CqFJLrT4rSpA46RQkVYWn8tdBDuQ7p7RXcp6Um76oaph").unwrap();
        tracing::debug!("[start_trading_transaction] Pyth oracle: {}", pyth_oracle);
        
        let switchboard_oracle = Pubkey::from_str("7azgmy1pFXHikv36q1zZASvFq5vFa39TT9NweVugKKTU").unwrap();
        tracing::debug!("[start_trading_transaction] Switchboard oracle: {}", switchboard_oracle);
        
        // USDC Reserve related accounts for devnet
        tracing::info!("[start_trading_transaction] Setting up USDC reserve accounts");
        let reserve_liquidity_supply = Pubkey::from_str("8SheGtsopRUDzdiD6v6BR9a6bqZ9QwywYQY99Fp5meNf").unwrap(); // USDC reserve liquidity supply
        tracing::debug!("[start_trading_transaction] Reserve liquidity supply: {}", reserve_liquidity_supply);
        
        let reserve_collateral_mint = Pubkey::from_str("FzwZWRMc3GCqjSrcpVX3ueJc6UpcV6iWWb7ZMsTXE3Gf").unwrap(); // cUSDC mint
        tracing::debug!("[start_trading_transaction] Reserve collateral mint: {}", reserve_collateral_mint);
        
        let lending_market_authority = Pubkey::from_str("DdZR6zRFiUt4S5mg7AV1uKB2z1f1WzcNYCaTEEWPAuby").unwrap(); // Lending market authority
        tracing::debug!("[start_trading_transaction] Lending market authority: {}", lending_market_authority);
        
        // Your bucket's vault token account (source of USDC to lend)
        tracing::info!("[start_trading_transaction] Setting up token accounts");
        let usdc_mint = Pubkey::from_str("2RgRJx3z426TMCL84ZMXTRVCS5ee7iGVE4ogqcUAd3tg").unwrap(); // Your USDC mint from tests
        tracing::debug!("[start_trading_transaction] USDC mint: {}", usdc_mint);
        
        let source_liquidity = get_associated_token_address(&bucket_pda, &usdc_mint);
        tracing::info!("[start_trading_transaction] Source liquidity (bucket's USDC vault): {}", source_liquidity);
        
        // Destination for cUSDC tokens (collateral tokens you'll receive)
        let destination_collateral = get_associated_token_address(&bucket_pda, &reserve_collateral_mint);
        tracing::info!("[start_trading_transaction] Destination collateral (bucket's cUSDC account): {}", destination_collateral);

        tracing::info!("[start_trading_transaction] Building instruction with accounts");
        let start_trading_accounts = StartTradingAccount {
            bucket: bucket_pda,
            trading_pool: trading_pool_pda,
            creator,
            reserve,
            reserve_liquidity_supply,
            lending_market,
            lending_market_authority,
            destination_collateral,
            source_liquidity,
            reserve_collateral_mint,
            pyth_oracle,
            switchboard_oracle,
            token_program: spl_token::ID,
            solend_program,
        };
        tracing::debug!("[start_trading_transaction] All accounts prepared for StartTrading instruction");
        
        let start_trading_args = StartTrading {
            bucket_name: request.bucket_name.clone(),
        };
        tracing::debug!("[start_trading_transaction] StartTrading arguments: bucket_name={}", start_trading_args.bucket_name);
        
        let instruction = program
            .request()
            .accounts(start_trading_accounts)
            .args(start_trading_args)
            .instructions()?;
        tracing::info!("[start_trading_transaction] Instruction created successfully, {} instructions generated", instruction.len());

        tracing::info!("[start_trading_transaction] Fetching recent blockhash");
        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        tracing::debug!("[start_trading_transaction] Recent blockhash: {}", recent_blockhash);
        
        // Create UNSIGNED transaction for frontend signing
        tracing::info!("[start_trading_transaction] Creating unsigned transaction for frontend signing");
        let tx = Transaction::new_with_payer(
            &instruction,
            Some(&creator),
        );
        tracing::debug!("[start_trading_transaction] Transaction created with payer: {}", creator);
        
        // Serialize transaction for frontend
        tracing::info!("[start_trading_transaction] Serializing transaction");
        let serialized_tx = bincode::serialize(&tx)?;
        tracing::debug!("[start_trading_transaction] Transaction serialized, size: {} bytes", serialized_tx.len());
        
        let base64_tx = base64::encode(serialized_tx);
        tracing::debug!("[start_trading_transaction] Transaction encoded to base64, length: {} chars", base64_tx.len());

        let response = UnsignedTransactionResponse {
            transaction: base64_tx,
            message: format!("Start trading for bucket '{}'", request.bucket_name),
        };
        tracing::info!("[start_trading_transaction] Transaction creation completed successfully");
        tracing::debug!("[start_trading_transaction] Response message: {}", response.message);
        
        Ok(response)
}

    fn encode_response(&self, sig: String, message: String) -> UnsignedTransactionResponse {
        UnsignedTransactionResponse { transaction: sig, message }
    }
}
