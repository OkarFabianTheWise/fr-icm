// Add this constant for vault seed if not already present
pub const VAULT_SEED: &[u8] = b"vault";
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
use solana_sdk::transaction::Transaction;
use spl_associated_token_account::get_associated_token_address;

declare_program!(icm_program);
pub const ICM_PROGRAM_ID: Pubkey = icm_program::ID;

use icm_program::client::args::{CreateBucket, ContributeToBucket, StartTrading, ClaimRewards, CloseBucket};
use icm_program::client::accounts::{CreateBucket as CreateBucketAccount, ContributeToBucket as ContributeToBucketAccount, StartTrading as StartTradingAccount, SwapTokens as SwapTokensAccount, ClaimRewards as ClaimRewardsAccount, CloseBucket as CloseBucketAccount, CreateProfile as CreateProfileAccount};
pub use crate::state_structs::{TradingPool, CreatorProfile, BucketAccount, BucketInfo};
use crate::state_structs::{CreateBucketRequest, ContributeToBucketRequest, StartTradingRequest, SwapTokensRequest, ClaimRewardsRequest, CloseBucketRequest, UnsignedTransactionResponse, GetCreatorProfileQuery, GetBucketQuery};

#[derive(Debug, Clone)]
pub struct IcmProgramInstance {
    pub cluster: Cluster,
    pub payer_pubkey: Pubkey,
    // Add other fields as needed
}

impl IcmProgramInstance {
    /// Agent swap tokens transaction
    pub async fn agent_swap_tokens_transaction(
        &self,
        request: SwapTokensRequest,
        keypair: Keypair,
        bucket_name: &str,
        input_mint: Pubkey,
        output_mint: Pubkey,
        jupiter_program: Pubkey,
        token_2022_program: Pubkey,
        platform_fee_account: Pubkey,
        input_mint_program: Pubkey,
        output_mint_program: Pubkey,
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
                input_mint_program,
                output_mint,
                output_mint_program,
                vault_input_token_account,
                vault_output_token_account,
                jupiter_program,
                token_2022_program,
                platform_fee_account,
            })
            .args(icm_program::client::args::SwapTokens {
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
            message: format!("Agent swap tokens for '{}'", creator.to_string()),
        })
    }

    /// Swap tokens transaction for frontend signing
    pub async fn manual_swap_tokens_transaction(
        &self,
        request: SwapTokensRequest,
        keypair: Keypair,
        bucket_name: &str,
        input_mint: Pubkey,
        output_mint: Pubkey,
        jupiter_program: Pubkey,
        token_2022_program: Pubkey,
        platform_fee_account: Pubkey,
        input_mint_program: Pubkey,
        output_mint_program: Pubkey,
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
                input_mint_program,
                output_mint,
                output_mint_program,
                vault_input_token_account,
                vault_output_token_account,
                jupiter_program,
                token_2022_program,
                platform_fee_account,
            })
            .args(icm_program::client::args::SwapTokens {
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
            message: format!("Swap tokens for '{}'", creator.to_string()),
        })
    }

    /// Create a new instance of the ICM program client
    pub fn new(cluster: Cluster, payer: Keypair) -> Result<Self> {
        Ok(Self {
            cluster,
            payer_pubkey: payer.pubkey(),
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
        let usdc_mint = Pubkey::from_str("7efeK5MMfmgcNeJkutSduzBGskFHziBhvmoPcPrJBmuF")?;
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
                contribution_window_days: request.contribution_window_days,
                trading_window_days: request.trading_window_days,
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
        let token_mint = Pubkey::from_str("7efeK5MMfmgcNeJkutSduzBGskFHziBhvmoPcPrJBmuF")?;

        // Derive bucket PDA: [b"bucket", bucket_name, creator]
        let (bucket_pda, _) = Pubkey::find_program_address(
            &[b"bucket", request.bucket_name.as_bytes(), creator.as_ref()],
            &ICM_PROGRAM_ID
        );

        // Derive contribution record PDA
        let (contribution_record_pda, _) = Pubkey::find_program_address(
            &[b"contribution", bucket_pda.as_ref(), contributor.as_ref(), token_mint.as_ref()],
            &ICM_PROGRAM_ID
        );

        // Derive pool contribution PDA
        let (pool_contribution_pda, _) = Pubkey::find_program_address(
            &[b"pool_contribution", bucket_pda.as_ref(), contributor.as_ref(), token_mint.as_ref()],
            &ICM_PROGRAM_ID
        );

        // Derive contributor token account (ATA)
        let contributor_token_account = get_associated_token_address(&contributor, &token_mint);

        // Derive vault token account PDA: [b"vault", bucket_pda, token_mint]
        let vault_token_account = get_associated_token_address(&bucket_pda, &token_mint);

        let ixs = program
            .request()
            .accounts(ContributeToBucketAccount{
                bucket: bucket_pda,
                contribution_record: contribution_record_pda,
                pool_contribution: pool_contribution_pda,
                contributor_token_account,
                vault_token_account,
                token_mint,
                contributor,
                token_program: spl_token::ID,
                associated_token_program: spl_associated_token_account::ID,
                system_program: system_program::id(),
            })
            .args(ContributeToBucket {
                token_mint,
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
        keypair: Keypair
    ) -> Result<Vec<BucketInfo>> {
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
                    trading_started_at: anchor_bucket.trading_started_at.to_string(),
                    closed_at: anchor_bucket.closed_at.to_string(),
                    bump: anchor_bucket.bump,
                    creator_profile: anchor_bucket.creator_profile,
                    performance_fee: anchor_bucket.performance_fee,
                    raised_amount: anchor_bucket.raised_amount,
                    contributor_count: anchor_bucket.contributor_count,
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
            AccountMeta::new(bucket_pda, false),
            AccountMeta::new(trading_pool_pda, false),
            AccountMeta::new(creator, true),
        ];

        let ixs = program
            .request()
            .accounts(accounts)
            .args(StartTrading {})
            .instructions()?;
        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&creator),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;
        tracing::info!("Start trading transaction confirmed: {}", sig);

        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: format!("Start trading for bucket '{}'", request.bucket_name),
        })
    }

    fn encode_response(&self, sig: String, message: String) -> UnsignedTransactionResponse {
        UnsignedTransactionResponse { transaction: sig, message }
    }
}


