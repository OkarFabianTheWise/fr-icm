//! # Server Module
//!
//! HTTP server setup and route configuration for the ICM server.

use axum::{Router, routing::{get, post}};
use tokio::net::TcpListener;
use std::sync::Arc;
use anchor_client::Cluster;
use solana_sdk::signature::Keypair;
use tokio::sync::RwLock;

use crate::routes::health::ping;
use crate::routes::icm;
use crate::routes::agent;
use crate::onchain_instance::instance::IcmProgramInstance;
use crate::agent::TradingAgent;

/// Application state shared across all route handlers
#[derive(Clone)]
pub struct AppState {
    pub icm_client: Arc<IcmProgramInstance>,
    pub trading_agent: Arc<RwLock<Option<TradingAgent>>>,
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

    // Create application state
    let app_state = AppState {
        icm_client: icm_instance,
        trading_agent: Arc::new(RwLock::new(None)),
    };

    // Create the main application router with all route configurations
    let app = Router::new()
        .route("/ping", get(ping)) // Health check endpoint
        
        // ICM Program Routes
        .route("/api/v1/bucket", post(icm::create_bucket))
        .route("/api/v1/bucket", get(icm::get_bucket))
        .route("/api/v1/bucket/contribute", post(icm::contribute_to_bucket))
        .route("/api/v1/bucket/start-trading", post(icm::start_trading))
        .route("/api/v1/bucket/swap", post(icm::swap_tokens))
        .route("/api/v1/bucket/claim-rewards", post(icm::claim_rewards))
        .route("/api/v1/bucket/close", post(icm::close_bucket))
        
        // Merge agent routes
        .merge(agent::create_routes())
        
        .with_state(app_state);

    // Define the server address - currently localhost only for development
    let addr = "127.0.0.1:3000";

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
    axum::serve(listener, app).await.expect("Server failed to start or encountered a fatal error");
}
