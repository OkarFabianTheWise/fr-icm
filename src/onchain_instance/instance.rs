impl IcmProgramInstance {
    /// Fetch all pools (buckets) for internal use (not as an Axum handler)
    pub async fn fetch_all_pools(&self) -> anyhow::Result<Vec<BucketInfo>> {
        // You may want to pass a keypair or use a default/test keypair for read-only fetches
        // For now, use a new ephemeral keypair (not for signing transactions)
        let keypair = solana_sdk::signature::Keypair::new();
        self.get_all_pools(keypair).await
    }
}
// --- On-chain account structs matching Anchor definitions ---
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TradingPool {
    pub pool_id: String,
    pub pool_bump: u8,
    pub creator: String,
    pub token_bucket: Vec<String>,
    pub target_amount: String,
    pub min_contribution: String,
    pub max_contribution: String,
    pub trading_duration: String,
    pub created_at: String,
    pub fundraising_deadline: String,
    pub trading_start_time: Option<String>,
    pub trading_end_time: Option<String>,
    pub phase: String,
    pub raised_amount: String,
    pub contributor_count: u32,
    pub management_fee: u16,
    pub performance_fee: u16,
    pub trade_count: u32,
    pub last_trade_time: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreatorProfile {
    pub creator: String,
    pub pools_created: u32,
    pub successful_pools: u32,
    pub total_volume_managed: String,
    pub reputation_score: u32,
    pub created_at: String,
}

impl IcmProgramInstance {
    /// Fetch a TradingPool by PDA (public key)
    pub async fn fetch_trading_pool_by_pda(&self, pool_pda: &str) -> Result<TradingPool> {
        let cluster = self.cluster.clone();
        let client = Client::new(cluster, Arc::new(Keypair::new()));
        let program = client.program(ICM_PROGRAM_ID)?;
        let pda = Pubkey::from_str(pool_pda)?;
        let anchor_pool: icm_program::accounts::TradingPool = program.account(pda).await?;
        Ok(TradingPool {
            pool_id: pool_pda.to_string(),
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
            raised_amount: anchor_pool.raised_amount.to_string(),
            contributor_count: anchor_pool.contributor_count,
            management_fee: anchor_pool.management_fee,
            performance_fee: anchor_pool.performance_fee,
            trade_count: anchor_pool.trade_count,
            last_trade_time: anchor_pool.last_trade_time.map(|v| v.to_string()),
        })
    }

    /// Fetch a CreatorProfile by PDA (public key)
    pub async fn fetch_creator_profile_by_pda(&self, creator_profile_pda: &str) -> Result<CreatorProfile> {
        let cluster = self.cluster.clone();
        let client = Client::new(cluster, Arc::new(Keypair::new()));
        let program = client.program(ICM_PROGRAM_ID)?;
        let pda = Pubkey::from_str(creator_profile_pda)?;
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
}
/// Main ICM program client instance
#[derive(Debug, Clone)]
pub struct IcmProgramInstance {
    pub cluster: Cluster,
    pub payer_pubkey: Pubkey,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateBucketRequest {
    pub name: String,
    pub token_mints: Vec<String>,
    pub contribution_window_days: u32,
    pub trading_window_days: u32,
    pub creator_fee_percent: u16,
    pub creator_pubkey: String,
}

use anchor_client::{Client, Cluster};
use anchor_lang::prelude::*;
use std::sync::Arc;
use std::str::FromStr;
use anyhow::{anyhow, Result};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use solana_sdk::transaction::Transaction;

declare_program!(icm_program);
pub const ICM_PROGRAM_ID: Pubkey = icm_program::ID;

use icm_program::client::args::{CreateBucket, ContributeToBucket, StartTrading, SwapTokens, ClaimRewards, CloseBucket};


#[derive(Debug, Clone, serde::Deserialize)]
pub struct ContributeToBucketRequest {
    pub bucket_name: String,
    pub token_mint: String,
    pub amount: u64,
    pub contributor_pubkey: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StartTradingRequest {
    pub bucket_name: String,
    pub creator_pubkey: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SwapTokensRequest {
    pub bucket_pubkey: String,
    pub creator_pubkey: String,
    pub input_mint: String,
    pub output_mint: String,
    pub route_plan: Vec<u8>,
    pub in_amount: u64,
    pub quoted_out_amount: u64,
    pub slippage_bps: u16,
    pub platform_fee_bps: u16,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ClaimRewardsRequest {
    pub bucket_name: String,
    pub token_mint: String,
    pub contributor_pubkey: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CloseBucketRequest {
    pub bucket_name: String,
    pub creator_pubkey: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetBucketQuery {
    pub bucket_name: String,
    pub creator_pubkey: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BucketAccount {
    pub creator: String,
    pub name: String,
    pub token_mints: Vec<String>,
    pub contribution_deadline: String, // Use String for BN compatibility
    pub trading_deadline: String,
    pub creator_fee_percent: u16,
    pub status: String, // You may want to use a struct/enum for richer info
    pub total_contributions: String,
    pub trading_started_at: String,
    pub closed_at: String,
    pub bump: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BucketInfo {
    pub public_key: String,
    pub account: BucketAccount,

}

#[derive(Debug, serde::Serialize)]
pub struct UnsignedTransactionResponse {
    pub transaction: String,
    pub message: String,
}


impl IcmProgramInstance {
    /// Get all pools (buckets) - stub implementation
    pub async fn get_all_pools(&self, keypair: Keypair) -> Result<Vec<BucketInfo>> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        // Fetch all Bucket accounts
        let bucket_accounts = program.accounts::<icm_program::accounts::Bucket>(vec![]).await?;

        let buckets: Vec<BucketInfo> = bucket_accounts
            .into_iter()
            .map(|(pubkey, anchor_bucket)| BucketInfo {
                public_key: pubkey.to_string(),
                account: BucketAccount {
                    creator: anchor_bucket.creator.to_string(),
                    name: anchor_bucket.name.clone(),
                    token_mints: anchor_bucket.token_mints.iter().map(|pk| pk.to_string()).collect(),
                    contribution_deadline: anchor_bucket.contribution_deadline.to_string(),
                    trading_deadline: anchor_bucket.trading_deadline.to_string(),
                    creator_fee_percent: anchor_bucket.creator_fee_percent,
                    status: format!("{:?}", anchor_bucket.status),
                    total_contributions: anchor_bucket.total_contributions.to_string(),
                    trading_started_at: anchor_bucket.trading_started_at.to_string(),
                    closed_at: anchor_bucket.closed_at.to_string(),
                    bump: anchor_bucket.bump,
                },
            })
            .collect();

        for bucket in &buckets {
            println!("Bucket: {:?}", bucket);
        }
        Ok(buckets)
    }

    /// Start trading transaction for frontend signing
    pub async fn start_trading_transaction(
        &self,
        request: StartTradingRequest,
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

        let accounts = vec![
            AccountMeta::new(bucket_pda, true),
            AccountMeta::new(trading_pool_pda, true),
            AccountMeta::new(creator, true),
        ];

        let ixs = program
            .request()
            .accounts(accounts)
            .args(StartTrading {
                bucket_name: request.bucket_name.clone(),
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
            message: format!("Start trading for bucket '{}'", request.bucket_name),
        })
    }
    fn encode_response(&self, sig: String, message: String) -> UnsignedTransactionResponse {
        UnsignedTransactionResponse { transaction: sig, message }
    }
}


impl IcmProgramInstance {
    /// Create a new instance of the ICM program client
    pub fn new(cluster: Cluster, payer: Keypair) -> Result<Self> {
        Ok(Self {
            cluster,
            payer_pubkey: payer.pubkey(),
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

        let creator = Pubkey::from_str(&request.creator_pubkey).map_err(|e| anyhow!(e))?;
        let token_mints: Vec<Pubkey> = request.token_mints.iter().map(|m| Pubkey::from_str(m).unwrap()).collect();

        // Derive PDAs for bucket, trading_pool, creator_profile
        let (bucket_pda, _) = Pubkey::find_program_address(&[b"bucket", request.name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        let (trading_pool_pda, _) = Pubkey::find_program_address(&[b"trading_pool", request.name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        let (creator_profile_pda, _) = Pubkey::find_program_address(&[b"creator_profile", creator.as_ref()], &ICM_PROGRAM_ID);

        let accounts = vec![
            AccountMeta::new(bucket_pda, false),
            AccountMeta::new(trading_pool_pda, false),
            AccountMeta::new(creator_profile_pda, false),
            AccountMeta::new(creator, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ];

        let ixs = program
            .request()
            .accounts(accounts)
            .args(CreateBucket {
                name: request.name.clone(),
                token_mints: token_mints.clone(),
                contribution_window_days: request.contribution_window_days,
                trading_window_days: request.trading_window_days,
                creator_fee_percent: request.creator_fee_percent,
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

        let contributor = Pubkey::from_str(&request.contributor_pubkey).map_err(|e| anyhow!(e))?;
        let token_mint = Pubkey::from_str(&request.token_mint).map_err(|e| anyhow!(e))?;
        let (bucket_pda, _) = Pubkey::find_program_address(&[b"bucket", request.bucket_name.as_bytes(), contributor.as_ref()], &ICM_PROGRAM_ID);
        let (contribution_record_pda, _) = Pubkey::find_program_address(&[b"contribution", bucket_pda.as_ref(), contributor.as_ref(), token_mint.as_ref()], &ICM_PROGRAM_ID);
        let (pool_contribution_pda, _) = Pubkey::find_program_address(&[b"pool_contribution", bucket_pda.as_ref(), contributor.as_ref(), token_mint.as_ref()], &ICM_PROGRAM_ID);
        // TODO: Derive or fetch contributor_token_account, vault_token_account as needed
        let contributor_token_account = Pubkey::default(); // Replace with real ATA
        let vault_token_account = Pubkey::default(); // Replace with real vault

        let accounts = vec![
            AccountMeta::new(bucket_pda, false),
            AccountMeta::new(contribution_record_pda, false),
            AccountMeta::new(pool_contribution_pda, false),
            AccountMeta::new(contributor_token_account, false),
            AccountMeta::new(vault_token_account, false),
            AccountMeta::new(contributor, true),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ];

        let ixs = program
            .request()
            .accounts(accounts)
            .args(ContributeToBucket {
                bucket_name: request.bucket_name.clone(),
                token_mint,
                amount: request.amount,
            })
            .instructions()?;
        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let contributor = Pubkey::from_str(&request.contributor_pubkey).map_err(|e| anyhow!(e))?;
        let token_mint = Pubkey::from_str(&request.token_mint).map_err(|e| anyhow!(e))?;
        // Derive bucket PDA: seeds = [b"bucket", bucket_name, bucket.creator]
        // But we don't have bucket.creator directly, so fetch it or require it in request if needed. For now, assume contributor is not creator.
        // For demo, use contributor as creator (not correct for production)
        let (bucket_pda, _) = Pubkey::find_program_address(&[b"bucket", request.bucket_name.as_bytes(), contributor.as_ref()], &ICM_PROGRAM_ID);
        let (contribution_record_pda, _) = Pubkey::find_program_address(&[b"contribution", bucket_pda.as_ref(), contributor.as_ref(), token_mint.as_ref()], &ICM_PROGRAM_ID);
        let (pool_contribution_pda, _) = Pubkey::find_program_address(&[b"pool_contribution", bucket_pda.as_ref(), contributor.as_ref(), token_mint.as_ref()], &ICM_PROGRAM_ID);
        // Derive contributor_token_account (ATA)
        let contributor_token_account = spl_associated_token_account::get_associated_token_address(&contributor, &token_mint);
        // Derive vault_token_account (PDA, see IDL: seeds = [bucket, token_program, token_mint], program = const)
        let token_program_id = spl_token::ID;
        let vault_seeds: &[&[u8]] = &[b"vault", bucket_pda.as_ref(), token_mint.as_ref()];
        let (vault_token_account, _) = Pubkey::find_program_address(vault_seeds, &ICM_PROGRAM_ID);

        let accounts = vec![
            AccountMeta::new(bucket_pda, false),
            AccountMeta::new(contribution_record_pda, false),
            AccountMeta::new(pool_contribution_pda, false),
            AccountMeta::new(contributor_token_account, false),
            AccountMeta::new(vault_token_account, false),
            AccountMeta::new(contributor, true),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ];
        // ...existing code...
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = Pubkey::from_str(&request.contributor_pubkey).map_err(|e| anyhow!(e))?;
        let (bucket_pda, _) = Pubkey::find_program_address(&[b"bucket", request.bucket_name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        let (trading_pool_pda, _) = Pubkey::find_program_address(&[b"trading_pool", request.bucket_name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);

        let accounts = vec![
            AccountMeta::new(bucket_pda, true),
            AccountMeta::new(trading_pool_pda, true),
            AccountMeta::new(creator, true),
        ];

        let ixs = program
            .request()
            .accounts(accounts)
            .args(StartTrading {
                bucket_name: request.bucket_name.clone(),
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
            message: format!("Start trading for bucket '{}'", request.bucket_name),
        })
    }

    /// Swap tokens transaction for frontend signing
    pub async fn swap_tokens_transaction(
        &self,
        request: SwapTokensRequest,
        keypair: Keypair
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = Pubkey::from_str(&request.creator_pubkey).map_err(|e| anyhow!(e))?;
        let bucket = Pubkey::from_str(&request.bucket_pubkey).map_err(|e| anyhow!(e))?;
        let input_mint = Pubkey::from_str(&request.input_mint).map_err(|e| anyhow!(e))?;
        let output_mint = Pubkey::from_str(&request.output_mint).map_err(|e| anyhow!(e))?;
        // TODO: Derive all required PDAs and accounts per IDL
        let trade_record = Pubkey::default();
        let vault_input_token_account = Pubkey::default();
        let vault_output_token_account = Pubkey::default();
        let platform_fee_account = Pubkey::default();
        let jupiter_program = Pubkey::default();
        let token_2022_program = Pubkey::default();

        let accounts = vec![
            AccountMeta::new(trade_record, true),
            AccountMeta::new(creator, true),
            AccountMeta::new(bucket, true),
            AccountMeta::new(input_mint, false),
            AccountMeta::new_readonly(system_program::id(), false),
            // ...add all other required accounts from IDL...
        ];

        let ixs = program
            .request()
            .accounts(accounts)
            .args(SwapTokens {
                route_plan: request.route_plan.clone(),
                in_amount: request.in_amount,
                quoted_out_amount: request.quoted_out_amount,
                slippage_bps: request.slippage_bps,
                platform_fee_bps: request.platform_fee_bps,
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
            message: format!("Swap {} tokens in bucket", request.in_amount),
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

        let contributor = Pubkey::from_str(&request.contributor_pubkey).map_err(|e| anyhow!(e))?;
        let token_mint = Pubkey::from_str(&request.token_mint).map_err(|e| anyhow!(e))?;
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
                bucket_name: request.bucket_name.clone(),
                token_mint,
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
        // ...existing code...
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
            .args(CloseBucket {
                bucket_name: request.bucket_name.clone(),
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
            message: format!("Close bucket '{}'", request.bucket_name),
        })
    }
}
    
