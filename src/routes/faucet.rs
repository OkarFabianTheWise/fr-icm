use axum::{Json, extract::{State, Request}, response::IntoResponse};
use axum_extra::extract::cookie::CookieJar;
use serde::{Deserialize, Serialize};
use crate::server::AppState;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};
use bs58;
use solana_sdk::system_instruction;
use std::{
    str::FromStr,
    sync::Arc,
    env
};

#[derive(Deserialize)]
pub struct FaucetRequest {
    pub amount: f64, // Human-readable USDC amount
}

#[derive(Serialize)]
pub struct FaucetResponse {
    pub success: bool,
    pub message: String,
    pub tx_signature: Option<String>,
}

const USDC_MINT: &str = "2RgRJx3z426TMCL84ZMXTRVCS5ee7iGVE4ogqcUAd3tg";
const MAX_FAUCET_AMOUNT: f64 = 100.0; // 100 USDC (human-readable)
const FAUCET_INTERVAL_SECS: u64 = 3 * 60 * 60; // 3 hours

/// Convert human-readable USDC amount to lamports (multiply by 1e6)
fn usdc_to_lamports(usdc_amount: f64) -> u64 {
    (usdc_amount * 1_000_000.0) as u64
}


#[axum::debug_handler]
pub async fn claim_faucet(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    // req: Request<axum::body::Body>,
    Json(body): Json<FaucetRequest>,
) -> axum::response::Response {
    // Extract JWT from cookie
    let token = jar.get("access_token").map(|c| c.value().to_string());
    if token.is_none() {
        return Json(FaucetResponse {
            success: false,
            message: "No access_token cookie found".to_string(),
            tx_signature: None,
        }).into_response();
    }
    let token = token.unwrap();
    // Validate token and get claims
    let claims = match state.jwt_service.decode_claims(&token) {
        Ok(c) => c,
        Err(_) => {
            return Json(FaucetResponse {
                success: false,
                message: "Invalid token".to_string(),
                tx_signature: None,
            }).into_response();
        }
    };
    let email = claims.email;
    // Fetch user profile from DB by email
    let user_profile = match state.db.get_user_profile_by_email(&email).await {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return Json(FaucetResponse {
                success: false,
                message: "User profile not found".to_string(),
                tx_signature: None,
            }).into_response();
        },
        Err(e) => {
            return Json(FaucetResponse {
                success: false,
                message: format!("DB error: {}", e),
                tx_signature: None,
            }).into_response();
        }
    };
    let user_pubkey = match Pubkey::from_str(&user_profile.user_pubkey) {
        Ok(pk) => pk,
        Err(_) => {
            return Json(FaucetResponse {
                success: false,
                message: "Invalid user pubkey in profile".to_string(),
                tx_signature: None,
            }).into_response();
        }
    };

    // log variables request+user data
    tracing::debug!("User profile: {:?}", user_profile);

    let req = body;
    if req.amount > MAX_FAUCET_AMOUNT {
        return Json(FaucetResponse {
            success: false,
            message: format!("Max faucet amount is {} USDC", MAX_FAUCET_AMOUNT),
            tx_signature: None,
        }).into_response();
    }
    // Rate limit: 3 hours per user (DB-backed)
    let now = chrono::Utc::now().naive_utc();
    if let Some(last_claim) = user_profile.last_faucet_claim {
        let elapsed = (now - last_claim).num_seconds();
        if elapsed < FAUCET_INTERVAL_SECS as i64 {
            let wait = FAUCET_INTERVAL_SECS as i64 - elapsed;
            return Json(FaucetResponse {
                success: false,
                message: format!("Faucet can be claimed again in {} seconds", wait),
                tx_signature: None,
            }).into_response();
        }
    }
    // Send USDC to user
    let faucet_private_key = env::var("FAUCET_PRIVATE_KEY").unwrap_or_default();
    let faucet_keypair = match bs58::decode(&faucet_private_key).into_vec() {
        Ok(bytes) => match Keypair::from_bytes(&bytes) {
            Ok(kp) => Arc::new(kp),
            Err(_) => {
                return Json(FaucetResponse {
                    success: false,
                    message: "Faucet keypair error (invalid bytes)".to_string(),
                    tx_signature: None,
                }).into_response();
            }
        },
        Err(_) => {
            return Json(FaucetResponse {
                success: false,
                message: "Faucet keypair error (invalid base58)".to_string(),
                tx_signature: None,
            }).into_response();
        }
    };

    tracing::debug!("Faucet public key: {:?}", faucet_keypair.pubkey());

    let usdc_mint = Pubkey::from_str(USDC_MINT).unwrap();
    let cluster = state.icm_client.cluster.clone();
    let client = anchor_client::Client::new_with_options(cluster, faucet_keypair.clone(), anchor_client::solana_sdk::commitment_config::CommitmentConfig::processed());
    let program = client.program(crate::onchain_instance::instance::ICM_PROGRAM_ID).unwrap();
    let rpc = program.rpc();

    // Derive faucet and user ATAs
    let faucet_ata = spl_associated_token_account::get_associated_token_address(&faucet_keypair.pubkey(), &usdc_mint);
    let user_ata = spl_associated_token_account::get_associated_token_address(&user_pubkey, &usdc_mint);
    // tracing::info!("Faucet ATA: {}", faucet_ata);
    // tracing::info!("User ATA: {}", user_ata);
    
    // Check if user's ATA exists, create if not
    let mut instructions = Vec::new();
    
    // Check if user ATA exists
    let user_ata_account = rpc.get_account(&user_ata).await;
    if user_ata_account.is_err() {
        // Create user's ATA if it doesn't exist
        let create_ata_ix = spl_associated_token_account::instruction::create_associated_token_account(
            &faucet_keypair.pubkey(), // payer
            &user_pubkey,             // wallet
            &usdc_mint,               // mint
            &spl_token::ID,           // token program
        );
        instructions.push(create_ata_ix);
    }
    
    // Build USDC transfer instruction
    let usdc_ix = spl_token::instruction::transfer(
        &spl_token::ID,
        &faucet_ata,
        &user_ata,
        &faucet_keypair.pubkey(),
        &[],
        usdc_to_lamports(req.amount),
    ).unwrap();
    instructions.push(usdc_ix);

    // Build SOL airdrop instruction (0.05 SOL = 50_000_000 lamports)
    let sol_airdrop_lamports = 50_000_000u64;
    let sol_ix = system_instruction::transfer(
        &faucet_keypair.pubkey(),
        &user_pubkey,
        sol_airdrop_lamports,
    );
    
    // Add SOL transfer to the beginning of instructions
    instructions.insert(0, sol_ix);
    
    let recent_blockhash = match rpc.get_latest_blockhash().await {
        Ok(b) => b,
        Err(e) => {
            return Json(FaucetResponse {
                success: false,
                message: format!("Blockhash error: {}", e),
                tx_signature: None,
            }).into_response();
        }
    };
    // Create transaction with all instructions
    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &instructions,
        Some(&faucet_keypair.pubkey()),
        &[faucet_keypair.as_ref()],
        recent_blockhash,
    );

    // tracing::info!("Faucet transaction: {:?}", tx);

    match rpc.send_and_confirm_transaction(&tx).await {
        Ok(sig) => {
            // Update last_faucet_claim in DB
            let _ = state.db.update_last_faucet_claim(&user_profile.user_pubkey, now).await;
            Json(FaucetResponse {
                success: true,
                message: "Faucet claim successful".to_string(),
                tx_signature: Some(sig.to_string()),
            }).into_response()
        },
        Err(e) => {
            Json(FaucetResponse {
                success: false,
                message: format!("Faucet transfer failed: {}", e),
                tx_signature: None,
            }).into_response()
        }
    }
}

