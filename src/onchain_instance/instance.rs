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
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
};
use solana_sdk::transaction::Transaction;
use spl_associated_token_account::get_associated_token_address;

declare_program!(icm_program);
pub const ICM_PROGRAM_ID: Pubkey = icm_program::ID;
pub const VAULT_SEED: &[u8] = b"vault";

use icm_program::client::args::{CreateBucket, ContributeToBucket, StartTrading, ClaimRewards, CloseBucket, InitializeProgram};
use icm_program::client::accounts::{CreateBucket as CreateBucketAccount, ContributeToBucket as ContributeToBucketAccount, StartTrading as StartTradingAccount, SwapTokens as SwapTokensAccount, ClaimRewards as ClaimRewardsAccount, CloseBucket as CloseBucketAccount, CreateProfile as CreateProfileAccount, InitializeProgram as InitializeProgramAccount};
pub use crate::state_structs::{TradingPool, CreatorProfile, BucketAccount, BucketInfo};
use crate::state_structs::{CreateBucketRequest, ContributeToBucketRequest, StartTradingRequest, SwapTokensRequest, ClaimRewardsRequest, CloseBucketRequest, InitializeProgramRequest, UnsignedTransactionResponse, GetCreatorProfileQuery, GetBucketQuery};

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
    
    /// Check if the program is initialized
    pub async fn check_program_initialized(&self, usdc_mint: Pubkey) -> Result<bool> {
        let cluster = self.cluster.clone();
        let dummy_keypair = Keypair::new(); // We just need this for the client, won't be used for signing
        let client = Client::new_with_options(cluster, Arc::new(dummy_keypair), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        // Derive program_state PDA
        let (program_state_pda, _) = Pubkey::find_program_address(
            &[b"program_state"],
            &ICM_PROGRAM_ID,
        );

        match program.account::<icm_program::accounts::ProgramState>(program_state_pda).await {
            Ok(program_state_account) => {
                Ok(program_state_account.initialized && program_state_account.usdc_mint == usdc_mint)
            },
            Err(_) => Ok(false)
        }
    }
    
    /// Initialize the program state (must be called first before any other operations)
    pub async fn initialize_program_transaction(
        &self,
        request: InitializeProgramRequest,
        owner_keypair: Keypair,
    ) -> Result<UnsignedTransactionResponse> {
        let cluster = self.cluster.clone();
        let keypair_for_client = owner_keypair.insecure_clone();
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let owner = owner_keypair.pubkey();
        let usdc_mint = Pubkey::from_str(&request.usdc_mint)?;

        // Derive program_state PDA
        let (program_state_pda, _) = Pubkey::find_program_address(
            &[b"program_state"],
            &ICM_PROGRAM_ID,
        );

        // Derive fee_vault PDA (ATA for program_state and USDC mint)
        let fee_vault = get_associated_token_address(&program_state_pda, &usdc_mint);

        tracing::info!("=== INITIALIZE PROGRAM DEBUG INFO ===");
        tracing::info!("Owner: {}", owner);
        tracing::info!("USDC Mint: {}", usdc_mint);
        tracing::info!("Program State PDA: {}", program_state_pda);
        tracing::info!("Fee Vault: {}", fee_vault);
        tracing::info!("Fee Rate BPS: {}", request.fee_rate_bps);

        let ixs = program
            .request()
            .args(InitializeProgram {
                fee_rate_bps: request.fee_rate_bps,
            })
            .accounts(InitializeProgramAccount {
                program_state: program_state_pda,
                fee_vault,
                usdc_mint,
                owner,
                token_program: spl_token::ID,
                associated_token_program: spl_associated_token_account::ID,
                system_program: system_program::ID,
            })
            .instructions()?;

        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&owner),
            &[&owner_keypair],
            recent_blockhash,
        );
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;

        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: "Program initialized successfully".to_string(),
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
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::confirmed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = keypair.pubkey();
        let (creator_profile_pda, _) = Pubkey::find_program_address(&[b"creator_profile", creator.as_ref()], &ICM_PROGRAM_ID);

        tracing::info!("[create_profile_transaction] Creating profile for creator: {}", creator);
        tracing::info!("[create_profile_transaction] Creator profile PDA: {}", creator_profile_pda);

        let mut ixs = program
            .request()
            .accounts(CreateProfileAccount {
                creator_profile: creator_profile_pda,
                creator,
                system_program: system_program::id(),
            })
            .args(icm_program::client::args::CreateProfile {})
            .instructions()?;

        // Add compute budget instruction
        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(300_000);
        ixs.insert(0, compute_budget_ix);

        let recent_blockhash = program.rpc().get_latest_blockhash().await?;
        let tx = Transaction::new_signed_with_payer(
            &ixs,
            Some(&creator),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        tracing::info!("[create_profile_transaction] Sending and confirming transaction...");
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
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::confirmed());
        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = keypair.pubkey();
        let token_mints: Vec<Pubkey> = request.token_mints.iter().map(|m| Pubkey::from_str(m).unwrap()).collect();

        // Derive PDAs for bucket, trading_pool, creator_profile
        let (bucket_pda, _) = Pubkey::find_program_address(&[b"bucket", request.name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        let (trading_pool_pda, _) = Pubkey::find_program_address(&[b"trading_pool", request.name.as_bytes(), creator.as_ref()], &ICM_PROGRAM_ID);
        let (creator_profile_pda, _) = Pubkey::find_program_address(&[b"creator_profile", creator.as_ref()], &ICM_PROGRAM_ID);
        let usdc_mint = Pubkey::from_str("2RgRJx3z426TMCL84ZMXTRVCS5ee7iGVE4ogqcUAd3tg")?;
        
        tracing::info!("=== CREATE BUCKET DEBUG INFO ===");
        tracing::info!("Bucket name: {}", request.name);
        tracing::info!("Creator: {}", creator);
        tracing::info!("Bucket PDA: {}", bucket_pda);
        tracing::info!("Trading Pool PDA: {}", trading_pool_pda);
        tracing::info!("Creator Profile PDA: {}", creator_profile_pda);
        
        // Verify creator profile exists before proceeding, create if it doesn't
        match program.account::<icm_program::accounts::CreatorProfile>(creator_profile_pda).await {
            Ok(_) => {
                tracing::info!("Creator profile verified - exists on chain");
            },
            Err(e) => {
                tracing::warn!("Creator profile does NOT exist on chain: {}. Creating it now...", e);
                
                // Create the profile
                let keypair_for_profile = keypair.insecure_clone();
                match self.create_profile_transaction(keypair_for_profile).await {
                    Ok(profile_response) => {
                        tracing::info!("Creator profile created successfully with signature: {}", profile_response.transaction);
                        
                        // Wait for confirmation
                        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                        
                        // Verify it was created
                        match program.account::<icm_program::accounts::CreatorProfile>(creator_profile_pda).await {
                            Ok(_) => {
                                tracing::info!("Creator profile verified after creation");
                            },
                            Err(verify_err) => {
                                tracing::error!("Failed to verify creator profile after creation: {}", verify_err);
                                return Err(anyhow!("Created profile but verification failed: {}", verify_err));
                            }
                        }
                    },
                    Err(create_err) => {
                        let error_str = create_err.to_string();
                        // If profile already exists (race condition), continue
                        if error_str.contains("already in use") || error_str.contains("custom program error: 0x0") {
                            tracing::info!("Creator profile already exists (race condition), continuing");
                        } else {
                            tracing::error!("Failed to create creator profile: {}", create_err);
                            return Err(anyhow!("Failed to create creator profile: {}", create_err));
                        }
                    }
                }
            }
        }
        
        // Derive program_state PDA
        let (program_state_pda, _) = Pubkey::find_program_address(
            &[b"program_state"],
            &ICM_PROGRAM_ID,
        );
        
        // Derive vault token account (ATA for bucket and USDC mint)
        let vault_token_account = get_associated_token_address(&bucket_pda, &usdc_mint);
        
        // Derive creator token account (ATA for creator and USDC mint)
        let creator_token_account = get_associated_token_address(&creator, &usdc_mint);
        
        // Derive fee_vault PDA (ATA for program_state and USDC mint)
        let fee_vault = get_associated_token_address(&program_state_pda, &usdc_mint);

        let mut ixs = program
            .request()
            .accounts(CreateBucketAccount {
                bucket: bucket_pda,
                trading_pool: trading_pool_pda,
                creator_profile: creator_profile_pda,
                vault_token_account,
                creator_token_account,
                usdc_mint: usdc_mint,
                program_state: program_state_pda,
                fee_vault,
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
                management_fee: request.management_fee as u64,
            })
            .instructions()?;

        // Add compute budget instruction to request more compute units
        let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000);
        ixs.insert(0, compute_budget_ix);

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

        // Derive vault token account (ATA for bucket and USDC mint)
        let vault_token_account = get_associated_token_address(&bucket_pda, &usdc_mint);

        // Derive program_state PDA
        let (program_state_pda, _) = Pubkey::find_program_address(
            &[b"program_state"],
            &ICM_PROGRAM_ID,
        );

        // Derive fee_vault PDA (ATA for program_state and USDC mint)
        let fee_vault = get_associated_token_address(&program_state_pda, &usdc_mint);

        // Add debugging info
        tracing::info!("=== CONTRIBUTE TO BUCKET DEBUG INFO ===");
        tracing::info!("Bucket name: {}", request.bucket_name);
        tracing::info!("Amount: {} lamports", request.amount);
        tracing::info!("Creator: {}", creator);
        tracing::info!("Contributor: {}", contributor);
        tracing::info!("USDC Mint: {}", usdc_mint);
        tracing::info!("Bucket PDA: {}", bucket_pda);
        tracing::info!("Contribution Record PDA: {}", contribution_record_pda);
        tracing::info!("Pool Contribution PDA: {}", pool_contribution_pda);
        tracing::info!("Contributor Token Account: {}", contributor_token_account);
        tracing::info!("Vault Token Account: {}", vault_token_account);
        tracing::info!("Program State PDA: {}", program_state_pda);
        tracing::info!("Fee Vault: {}", fee_vault);
        
        // Try to fetch the bucket account to verify it exists and check its state
        match program.account::<icm_program::accounts::Bucket>(bucket_pda).await {
            Ok(bucket_account) => {
                tracing::info!("Bucket exists with status: {:?}", bucket_account.status);
                tracing::info!("Bucket creator: {}", bucket_account.creator);
                tracing::info!("Bucket raised amount: {}", bucket_account.raised_amount);
                tracing::info!("Bucket contribution deadline: {}", bucket_account.contribution_deadline);
                
                // Verify the bucket creator matches the expected creator
                if bucket_account.creator != creator {
                    return Err(anyhow!("Bucket creator mismatch. Expected: {}, Found: {}", creator, bucket_account.creator));
                }
                
                // Check if contribution window is still open
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64;
                    
                if current_time >= bucket_account.contribution_deadline {
                    return Err(anyhow!("Contribution deadline has passed"));
                }
            },
            Err(e) => {
                tracing::error!("Failed to fetch bucket account: {}", e);
                return Err(anyhow!("Bucket does not exist or cannot be fetched: {}", e));
            }
        }
        
        // Check if program state exists and is initialized
        match program.account::<icm_program::accounts::ProgramState>(program_state_pda).await {
            Ok(program_state_account) => {
                tracing::info!("Program state initialized: {}", program_state_account.initialized);
                tracing::info!("Program state USDC mint: {}", program_state_account.usdc_mint);
                
                if !program_state_account.initialized {
                    return Err(anyhow!("Program state is not initialized"));
                }
                
                if program_state_account.usdc_mint != usdc_mint {
                    return Err(anyhow!("USDC mint mismatch. Expected: {}, Found: {}", usdc_mint, program_state_account.usdc_mint));
                }
            },
            Err(e) => {
                tracing::error!("Failed to fetch program state: {}", e);
                return Err(anyhow!("Program state does not exist or cannot be fetched: {}. Please initialize the program first by calling the /api/v1/program/initialize endpoint.", e));
            }
        }
        
        // Check if contributor has sufficient balance
        match program.rpc().get_token_account_balance(&contributor_token_account).await {
            Ok(balance) => {
                let balance_lamports = balance.amount.parse::<u64>().unwrap_or(0);
                tracing::info!("Contributor token balance: {} lamports", balance_lamports);
                
                if balance_lamports < request.amount {
                    return Err(anyhow!("Insufficient token balance. Required: {}, Available: {}", request.amount, balance_lamports));
                }
            },
            Err(e) => {
                tracing::warn!("Could not fetch contributor token balance: {}", e);
                // Don't fail here as the account might not exist yet (will be created by ATA program)
            }
        }

        // Verify vault token account exists (should have been created during create_bucket)
        match program.rpc().get_account(&vault_token_account).await {
            Ok(account) => {
                tracing::info!("Vault token account exists with {} lamports", account.lamports);
            },
            Err(e) => {
                tracing::error!("Vault token account does not exist: {}", e);
                return Err(anyhow!("Vault token account does not exist. Make sure the bucket was created properly. Account: {}", vault_token_account));
            }
        }

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
                // bucket_name: request.bucket_name.clone(),
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
            management_fee: anchor_pool.management_fee as u16,
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
                        performance_fee: anchor_bucket.performance_fee as u16,
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

    /// Start trading transaction - signs and submits transaction server-side
    pub async fn start_trading_transaction(
        &self,
        request: StartTradingRequest,
        keypair: Keypair
    ) -> Result<UnsignedTransactionResponse> {
        tracing::error!("[start_trading_transaction] 🔥 FUNCTION CALLED - This should appear in logs!");
        tracing::info!("[start_trading_transaction] Starting transaction creation and submission");

        let cluster = self.cluster.clone();

        let keypair_for_client = keypair.insecure_clone();
        let keypair_for_sign = keypair.insecure_clone();
        
        let client = Client::new_with_options(cluster, Arc::new(keypair_for_client), CommitmentConfig::processed());

        let program = client.program(ICM_PROGRAM_ID)?;

        let creator = Pubkey::from_str(&request.creator_pubkey).map_err(|e| anyhow!(e))?;
        
        let (bucket_pda, bucket_bump) = Pubkey::find_program_address(
            &[b"bucket", request.bucket_name.as_bytes(), creator.as_ref()], 
            &ICM_PROGRAM_ID
        );
        
        let (trading_pool_pda, trading_pool_bump) = Pubkey::find_program_address(
            &[b"trading_pool", request.bucket_name.as_bytes(), creator.as_ref()], 
            &ICM_PROGRAM_ID
        );

        let start_trading_accounts = StartTradingAccount {
            bucket: bucket_pda,
            trading_pool: trading_pool_pda,
            creator,
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
        
        // Create signed transaction (server-side signing like other methods)
        let tx = Transaction::new_signed_with_payer(
            &instruction,
            Some(&creator),
            &[&keypair_for_sign],
            recent_blockhash,
        );
        
        tracing::info!("[start_trading_transaction] Transaction signed successfully");
        
        // Submit transaction to blockchain
        let sig = program.rpc().send_and_confirm_transaction(&tx).await?;
        tracing::info!("[start_trading_transaction] Transaction confirmed with signature: {}", sig);

        Ok(UnsignedTransactionResponse {
            transaction: sig.to_string(),
            message: format!("Start trading for bucket '{}'", request.bucket_name),
        })
}

    fn encode_response(&self, sig: String, message: String) -> UnsignedTransactionResponse {
        UnsignedTransactionResponse { transaction: sig, message }
    }
}
