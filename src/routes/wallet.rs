//! # Wallet Routes
//!
//! This module handles wallet-related API endpoints including:
//! - Balance fetching (SOL and SPL tokens)
//! - Wallet validation and utilities
//!
//! All endpoints require authentication via JWT middleware.

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use tracing::{debug, error, info, warn};

use crate::server::AppState;

/// Request parameters for wallet balance endpoint
#[derive(Debug, Deserialize)]
pub struct WalletBalanceQuery {
    /// The wallet public key (base58 encoded)
    pub public_key: String,
}

/// Response structure for wallet balance
#[derive(Debug, Serialize)]
pub struct WalletBalanceResponse {
    /// SOL balance in human-readable format (not lamports)
    pub sol_balance: f64,
    /// USDC balance in human-readable format (not smallest unit)
    pub usdc_balance: f64,
    /// The public key that was queried
    pub public_key: String,
}

/// Error response structure
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Get wallet balance for SOL and USDC
/// 
/// This endpoint fetches both SOL and USDC balances for a given wallet address.
/// It replaces the frontend direct RPC calls to improve performance and centralize
/// blockchain interactions on the backend.
///
/// # Parameters
/// - `public_key`: The wallet address in base58 format
///
/// # Returns
/// - SOL balance in SOL units (not lamports)
/// - USDC balance in USDC units (not smallest denomination)
///
/// # Authentication
/// Requires valid JWT token
pub async fn get_wallet_balance(
    State(state): State<AppState>,
    Query(query): Query<WalletBalanceQuery>,
) -> Result<Json<WalletBalanceResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Fetching wallet balance for: {}", query.public_key);

    // Validate and parse the public key
    let pubkey = match Pubkey::from_str(&query.public_key) {
        Ok(pk) => pk,
        Err(e) => {
            warn!("Invalid public key provided: {} - Error: {}", query.public_key, e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid public key: {}", e),
                }),
            ));
        }
    };

    // Create RPC client
    let rpc_client = RpcClient::new(
        std::env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string())
    );

    // Fetch SOL balance
    let sol_balance = match rpc_client.get_balance(&pubkey) {
        Ok(balance_lamports) => {
            debug!("SOL balance in lamports: {}", balance_lamports);
            balance_lamports as f64 / 1e9 // Convert lamports to SOL
        }
        Err(e) => {
            error!("Failed to fetch SOL balance for {}: {}", query.public_key, e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to fetch SOL balance: {}", e),
                }),
            ));
        }
    };

    // Fetch USDC balance
    // Note: You'll need to define the USDC token mint address for your environment
    // For devnet, common USDC mint: Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr
    let usdc_mint = match std::env::var("USDC_MINT_ADDRESS") {
        Ok(mint) => match Pubkey::from_str(&mint) {
            Ok(pk) => pk,
            Err(e) => {
                warn!("Invalid USDC mint address in environment: {}", e);
                // Fallback to devnet USDC mint
                Pubkey::from_str("Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr").unwrap()
            }
        },
        Err(_) => {
            // Default to devnet USDC mint if not set
            Pubkey::from_str("Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr").unwrap()
        }
    };

    let usdc_balance = match get_token_balance(&rpc_client, &pubkey, &usdc_mint).await {
        Ok(balance) => balance,
        Err(e) => {
            warn!("Failed to fetch USDC balance for {}: {}", query.public_key, e);
            // Don't fail the entire request if USDC balance fails, just return 0
            0.0
        }
    };

    info!(
        "Balance fetched successfully for {}: SOL={}, USDC={}",
        query.public_key, sol_balance, usdc_balance
    );

    Ok(Json(WalletBalanceResponse {
        sol_balance,
        usdc_balance,
        public_key: query.public_key,
    }))
}

/// Helper function to get SOL and USDC balances for a wallet
/// This can be used by auth endpoints to include balance data
pub async fn fetch_wallet_balances(public_key_str: &str) -> Result<WalletBalanceResponse, String> {
    // Validate and parse the public key
    let pubkey = match Pubkey::from_str(public_key_str) {
        Ok(pk) => pk,
        Err(e) => {
            return Err(format!("Invalid public key: {}", e));
        }
    };

    // Create RPC client
    let rpc_client = RpcClient::new(
        std::env::var("SOLANA_RPC_URL")
            .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string())
    );

    // Fetch SOL balance
    let sol_balance = match rpc_client.get_balance(&pubkey) {
        Ok(balance_lamports) => {
            debug!("SOL balance in lamports: {}", balance_lamports);
            balance_lamports as f64 / 1e9 // Convert lamports to SOL
        }
        Err(e) => {
            warn!("Failed to fetch SOL balance for {}: {}", public_key_str, e);
            0.0
        }
    };

    // Fetch USDC balance
    let usdc_mint = match std::env::var("USDC_MINT_ADDRESS") {
        Ok(mint) => match Pubkey::from_str(&mint) {
            Ok(pk) => pk,
            Err(e) => {
                warn!("Invalid USDC mint address in environment: {}", e);
                // Fallback to devnet USDC mint
                Pubkey::from_str("Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr").unwrap()
            }
        },
        Err(_) => {
            // Default to devnet USDC mint if not set
            Pubkey::from_str("Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr").unwrap()
        }
    };

    let usdc_balance = match get_token_balance(&rpc_client, &pubkey, &usdc_mint).await {
        Ok(balance) => balance,
        Err(e) => {
            warn!("Failed to fetch USDC balance for {}: {}", public_key_str, e);
            0.0
        }
    };

    Ok(WalletBalanceResponse {
        sol_balance,
        usdc_balance,
        public_key: public_key_str.to_string(),
    })
}

/// Helper function to get SPL token balance
async fn get_token_balance(
    rpc_client: &RpcClient,
    wallet_pubkey: &Pubkey,
    token_mint: &Pubkey,
) -> Result<f64, Box<dyn std::error::Error>> {
    use solana_client::rpc_filter::{Memcmp, RpcFilterType};
    use solana_sdk::program_pack::Pack;
    use spl_token::state::Account as TokenAccount;

    // Find the associated token account
    let token_accounts = rpc_client.get_token_accounts_by_owner(
        wallet_pubkey,
        solana_client::rpc_request::TokenAccountsFilter::Mint(*token_mint),
    )?;

    if token_accounts.is_empty() {
        debug!("No token account found for mint {} and wallet {}", token_mint, wallet_pubkey);
        return Ok(0.0);
    }

    // Get the first (and typically only) token account
    let token_account = &token_accounts[0];
    
    // Parse the token account data
    let token_account_pubkey = match Pubkey::from_str(&token_account.pubkey) {
        Ok(pk) => pk,
        Err(e) => {
            debug!("Invalid token account pubkey: {}", e);
            return Ok(0.0);
        }
    };
    
    let account_data = match rpc_client.get_account_data(&token_account_pubkey) {
        Ok(data) => data,
        Err(e) => {
            debug!("Failed to get token account data: {}", e);
            return Ok(0.0);
        }
    };

    let token_account_info = TokenAccount::unpack(&account_data)?;
    
    // Convert to human-readable format (USDC has 6 decimals)
    let balance = token_account_info.amount as f64 / 1e6;
    
    debug!("Token balance for mint {}: {}", token_mint, balance);
    Ok(balance)
}

/// Create the wallet routes
pub fn create_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/wallet/balance", get(get_wallet_balance))
}