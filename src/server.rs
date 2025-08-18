//! # Server Module
//!
//! HTTP server setup and route configuration for the ICM server.

use axum::{Router, routing::{get, post}};
use tower_http::cors::{CorsLayer};
use tokio::net::TcpListener;
use std::sync::Arc;
use anchor_client::Cluster;
use solana_sdk::signature::Keypair;
use tokio::sync::RwLock;

use crate::routes::health::ping;
use crate::routes::agent;
use crate::onchain_instance::instance::IcmProgramInstance;
use crate::agent::TradingAgent;

/// Application state shared across all route handlers
#[derive(Clone)]
pub struct AppState {
    pub icm_client: Arc<IcmProgramInstance>,
    pub trading_agent: Arc<RwLock<Option<TradingAgent>>>,
    pub jwt_service: Arc<crate::auth::jwt::JwtService>,
    pub db: Arc<crate::database::connection::DatabaseConnection>,
}

/// Starts the ICM (Intelligent Content Management) HTTP server.
///
/// This function initializes and starts the web server with all configured routes.
/// The server binds to localhost on port 3000 and serves the application using
/// the Axum web framework with Tokio runtime.
pub async fn start() {
    // Initialize the ICM program instance
    let payer = Keypair::new(); // TODO: Load from secure storage in production
    let icm_instance = match IcmProgramInstance::new(Cluster::Devnet, payer) {
        Ok(instance) => Arc::new(instance),
        Err(e) => {
            tracing::error!("Failed to initialize ICM program instance: {}", e);
            panic!("Cannot start server without ICM program instance");
        }
    };

    // Create JWT service first
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev_secret".to_string());
    let jwt_service = Arc::new(crate::auth::jwt::JwtService::new(&jwt_secret));

    // Initialize database connection
    let db_config = crate::database::connection::DatabaseConfig::from_env().expect("Failed to load DB config from env");
    let db = Arc::new(crate::database::connection::DatabaseConnection::new(db_config).await.expect("Failed to connect to DB"));

    // Create application state
    let app_state = AppState {
        icm_client: icm_instance,
        trading_agent: Arc::new(RwLock::new(None)),
        jwt_service: jwt_service.clone(),
        db: db.clone(),
    };

    // Import the AuthMiddleware
    use crate::auth::middleware::AuthMiddleware;
    use axum::middleware;


    // Bucket endpoints require authentication
    let bucket_routes = Router::new()
        .route("/api/v1/bucket/create", post(crate::routes::icm::create_bucket))
        .route("/api/v1/bucket/contribute", post(crate::routes::icm::contribute_to_bucket))
        .route("/api/v1/bucket/start-trading", post(crate::routes::icm::start_trading))
        .route("/api/v1/bucket/swap", post(crate::routes::icm::swap_tokens))
        .route("/api/v1/bucket/claim-rewards", post(crate::routes::icm::claim_rewards))
        .route("/api/v1/bucket/close", post(crate::routes::icm::close_bucket))
        .route("/api/v1/bucket/all", get(crate::routes::icm::get_all_pools_by_pda))
        .route("/api/v1/bucket/trading_pools", post(crate::routes::icm::get_trading_pool_info))
        .layer(middleware::from_fn_with_state(jwt_service.clone(), AuthMiddleware::validate_token));

    // Faucet route (no auth)
    let faucet_routes = Router::new()
        .route("/api/v1/faucet/claim", post(crate::routes::faucet::claim_faucet))
        .layer(middleware::from_fn_with_state(jwt_service.clone(), AuthMiddleware::validate_token))
        .with_state(Arc::new(app_state.clone()));

    use tower::ServiceBuilder;
    // Main app router
    let app = Router::new()
        .route("/ping", get(ping)) // Health check endpoint
        .merge(bucket_routes)
        .merge(faucet_routes)
        // Merge agent routes
        .merge(agent::create_routes())
        // Merge auth routes
        .merge(crate::routes::auth::create_auth_routes())
        .layer(
            ServiceBuilder::new()
                .layer(
                    CorsLayer::new()
                        .allow_origin(["https://fr-icm-ui.vercel.app".parse::<axum::http::HeaderValue>().unwrap(), "http://localhost:3001".parse::<axum::http::HeaderValue>().unwrap()]) // Allow frontend origin
                        .allow_methods([
                            axum::http::Method::GET,
                            axum::http::Method::POST,
                            axum::http::Method::OPTIONS,
                        ])
                        .allow_headers([
                            axum::http::header::ORIGIN,
                            axum::http::header::CONTENT_TYPE,
                            axum::http::header::ACCEPT,
                            axum::http::header::AUTHORIZATION,
                        ])
                        .allow_credentials(true) // Allow cookies for auth
                )
        )
        .with_state(app_state);

    // Define the server address - use $PORT if set (Heroku), otherwise default to 3000
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    // Create a TCP listener bound to the specified address
    let listener = TcpListener::bind(addr).await.expect(
        "Failed to bind to address - port may already be in use"
    );

    // Log server startup information
    tracing::info!("🚀 ICM Server starting...");
    tracing::info!("📡 Listening on http://{}", addr);
    tracing::info!("🏥 Health check available at http://{}/ping", addr);
    tracing::info!("📊 ICM Program endpoints available at http://{}/api/v1/bucket/*", addr);
    tracing::info!("🤖 AI Trading Agent endpoints available at http://{}/api/v1/agent/*", addr);
    tracing::info!("🔧 Environment: Development");
    tracing::info!("🌐 Cluster: Devnet");

    // Start serving the application
    axum::serve(listener, app).await.unwrap();
}
