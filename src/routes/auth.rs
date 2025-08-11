//! Auth routes for registration, login, and user info

use axum::{Json, Router, routing::{post, get}, extract::State};
use axum_extra::extract::cookie::{CookieJar, Cookie, SameSite};
use axum::response::{Json as AxumJson, IntoResponse, Response};
use axum::http::{header, StatusCode};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use argon2::{Argon2, password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString}};
use argon2::password_hash::rand_core::OsRng;
use solana_sdk::signature::Keypair;
use solana_sdk::signature::Signer;
use tokio_postgres::types::ToSql;
use deadpool_postgres::Pool;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::auth::{jwt::JwtService, models::AuthUser};
use uuid::Uuid;
use crate::server::AppState;
use crate::database::connection::DatabaseConfig;
use chrono::Duration as ChronoDuration;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct Tokens {
    pub access_token: String,
    pub expires_at: i64,
}

#[derive(Serialize)]
pub struct RegisterResponse {
    pub user: AuthUser,
    pub tokens: Tokens,
    pub wallet_address: String,
}

// /api/auth/me handler: returns user info if JWT in cookie is valid
// Ensure tracing logs are visible: set RUST_LOG=info if not already set
// You can do this in main.rs or your entrypoint:
// std::env::set_var("RUST_LOG", "info");
// tracing_subscriber::fmt::init();
// Or set the environment variable before running: RUST_LOG=info cargo run
pub async fn me(
    State(app_state): State<AppState>,
    jar: CookieJar,
    req: axum::http::Request<axum::body::Body>,
) -> (StatusCode, AxumJson<Value>) {
    // Log all request headers
    let headers = req.headers();
    for (name, value) in headers.iter() {
        tracing::info!("Header: {}: {:?}", name, value);
    }
    tracing::info!("/api/auth/me called");
    let cookie_names: Vec<_> = jar.iter().map(|c| c.name().to_string()).collect();
    tracing::info!("Cookies received: {:?}", cookie_names);
    // Get token from cookie (should be 'access_token', not 'kapitarise_access_token')
    let token = jar.get("access_token").map(|c| c.value().to_string());
    if token.is_none() {
        tracing::warn!("No access_token cookie found");
        return (StatusCode::UNAUTHORIZED, AxumJson(serde_json::json!({"error": "No token"})));
    }
    let token = token.unwrap();
    tracing::info!("access_token cookie found: {}", token);

    // Validate token and get claims
    let claims = match app_state.jwt_service.decode_claims(&token) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("JWT decode error: {:?}", e);
            return (StatusCode::UNAUTHORIZED, AxumJson(serde_json::json!({"error": "Invalid token"})));
        }
    };

    // Fetch user info from DB
    let pool = app_state.db.pool();
    let client = match pool.get().await {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, AxumJson(serde_json::json!({"error": "DB error"}))),
    };
    let row = match client.query_opt(
        "SELECT user_pubkey, email FROM user_profiles WHERE email = $1",
        &[&claims.email],
    ).await {
        Ok(Some(row)) => row,
        _ => return (StatusCode::UNAUTHORIZED, AxumJson(serde_json::json!({"error": "User not found"}))),
    };

    let wallet_address: String = row.try_get("user_pubkey").unwrap_or_default();
    let email: String = row.try_get("email").unwrap_or_default();

    let user = AuthUser {
        id: claims.sub,
        email,
        // fill other fields as needed
    };

    // Fetch user's keypair for pool queries
    let keypair = match crate::routes::icm::get_user_keypair_by_email(&user.email, &app_state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("Failed to get user keypair: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, AxumJson(serde_json::json!({"error": "Failed to get user keypair"})));
        }
    };
    let all_pools = match app_state.icm_client.get_all_pools_by_pda(keypair).await {
        Ok(pools) => pools,
        Err(e) => {
            tracing::error!("Failed to fetch all pools: {}", e);
            vec![]
        }
    };
    let body = serde_json::json!({
        "user": user,
        "wallet_address": wallet_address,
        "pools": all_pools
    });
    (StatusCode::OK, AxumJson(body))
}

pub async fn register(
    State(app_state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let email = payload.email.trim().to_lowercase();
    let password = payload.password;
    let pool = app_state.db.pool();
    let client = pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if user already exists by email
    let row = client.query_opt(
        "SELECT user_pubkey FROM user_profiles WHERE email = $1",
        &[&email],
    ).await.map_err(|e| {
        tracing::error!("Failed to query user profile: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if row.is_some() {
        return Err(StatusCode::CONFLICT); // Email already registered
    }

    // Hash password using Argon2 v3+ API
    let mut rng = OsRng;
    let salt = SaltString::generate(&mut rng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .to_string();

    // Generate wallet
    let keypair = Keypair::new();
    let pubkey_str = keypair.pubkey().to_string();
    let privkey_bytes: Vec<i32> = keypair.to_bytes().iter().map(|b| *b as i32).collect();
    let user_id = Uuid::new_v4();
    client.execute(
        "INSERT INTO user_profiles (user_id,user_pubkey, private_key, email, password_hash) VALUES ($1, $2, $3, $4, $5)",
        &[&user_id, &pubkey_str, &privkey_bytes, &email, &password_hash],
    ).await.map_err(|e| {
        tracing::error!("Failed to insert user profile: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let user = AuthUser {
        id: user_id,
        email: email.clone(),
    };

    let access_token = app_state.jwt_service.create_token(user_id, email.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Decode the token to get expiration
    let claims = app_state.jwt_service.decode_claims(&access_token)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let expires_at = claims.exp;

    // Set cookie attributes for cross-site refresh persistence
    // Use a consistent cookie name for frontend and backend
    let mut cookie = Cookie::new("access_token", access_token.clone());
    cookie.set_http_only(true);
    cookie.set_secure(true);
    cookie.set_same_site(SameSite::None); // Lax is more compatible for most apps
    cookie.set_path("/");
    cookie.set_domain("localhost");
    // Set expiry to match JWT expiry
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let max_age = expires_at - now;
    if max_age > 0 {
        cookie.set_max_age(time::Duration::seconds(max_age));
    }

    // Return user info and wallet address, but NOT the token
    let body = json!({
        "user": user,
        "wallet_address": pubkey_str,
        "expires_at": expires_at
    });
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::SET_COOKIE, cookie.to_string())
        .header(header::CONTENT_TYPE, "application/json")
        .body(serde_json::to_string(&body).unwrap())
        .unwrap();
    Ok(response)
}

pub async fn login(
    State(app_state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let email = payload.email.trim().to_lowercase();
    let password = payload.password;
    let pool = app_state.db.pool();
    let client = pool.get().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Fetch user by email
    let row = client.query_opt(
        "SELECT user_pubkey, password_hash, user_id FROM user_profiles WHERE email = $1",
        &[&email],
    ).await.map_err(|e| {
        tracing::error!("Failed to query user profile: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let row = match row {
        Some(row) => row,
        None => {
            let body = serde_json::json!({"error": "User not found"});
            let response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::CONTENT_TYPE, "application/json")
                .body(serde_json::to_string(&body).unwrap())
                .unwrap();
            return Ok(response);
        }
    };

    let password_hash: String = row.try_get("password_hash").unwrap_or_default();
    let wallet_address: String = row.try_get("user_pubkey").unwrap_or_default();
    let user_id = row.try_get::<_, Uuid>("user_id").unwrap_or_else(|_| Uuid::new_v4());

    // Verify password using Argon2 v3+ API
    let parsed_hash = PasswordHash::new(&password_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let argon2 = Argon2::default();
    if argon2.verify_password(password.as_bytes(), &parsed_hash).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    let user = AuthUser {
        id: user_id,
        email: email.clone(),
    };

    let access_token = app_state.jwt_service.create_token(user_id, email.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let claims = app_state.jwt_service.decode_claims(&access_token)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let expires_at = claims.exp;

    // Fetch user's keypair for pool queries
    let keypair = match crate::routes::icm::get_user_keypair_by_email(&user.email, &app_state).await {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("Failed to get user keypair: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    let all_pools = match app_state.icm_client.get_all_pools_by_pda(keypair).await {
        Ok(pools) => pools,
        Err(e) => {
            tracing::error!("Failed to fetch all pools: {}", e);
            vec![]
        }
    };

    // Set cookie attributes for cross-site refresh persistence
    let mut cookie = Cookie::new("access_token", access_token.clone());
    cookie.set_http_only(false);
    cookie.set_secure(true);
    // cookie.set_same_site(SameSite::Lax); // Lax is more compatible for most apps
    cookie.set_same_site(SameSite::None); // cross origin
    cookie.set_path("/");
    // cookie.set_domain("localhost");
    // Set expiry to match JWT expiry
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let max_age = expires_at - now;
    if max_age > 0 {
        cookie.set_max_age(time::Duration::seconds(max_age));
    }

    // Return user info, wallet address, expiresAt, and pools
    let body = json!({
        "user": user,
        "wallet_address": wallet_address,
        "expires_at": expires_at,
        "pools": all_pools
    });
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::SET_COOKIE, cookie.to_string())
        .header(header::CONTENT_TYPE, "application/json")
        .body(serde_json::to_string(&body).unwrap())
        .unwrap();
    Ok(response)
}

pub async fn logout() -> impl IntoResponse {
    // For stateless JWT, just return 204. Client deletes token.
    StatusCode::NO_CONTENT
}

pub fn create_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
}
