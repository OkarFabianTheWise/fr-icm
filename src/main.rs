//! # ICM Server
//!
//! Intelligent Content Management Server - A high-performance HTTP API server
//! built with Rust, Axum, and Tokio for handling content management operations.
//!
//! ## Features
//! - Async/await HTTP server using Axum framework
//! - Structured logging with tracing
//! - Health check endpoints for monitoring
//! - Modular route organization
//! - AI-powered trading agent with OpenAI integration
//! - Jupiter DEX integration for Solana token swaps
//!
//! ## Architecture
//! The server is organized into modules:
//! - `server`: Core server initialization and configuration
//! - `config`: Environment variable configuration management
//! - `agent`: AI trading agent with multiple strategies
//! - `routes`: HTTP route handlers organized by functionality
//!   - `health`: Health check and monitoring endpoints
//!   - `icm`: ICM program interaction endpoints
//!   - `agent`: Trading agent control endpoints
//!
//! ## Environment Setup
//! Copy `.env.example` to `.env` and configure:
//! ```bash
//! cp .env.example .env
//! # Edit .env with your API keys
//! ```
//!
//! ## Running the Server
//! ```bash
//! cargo run
//! ```
//!
//! The server will start on `http://127.0.0.1:3000` by default.
//!
//! ## Health Check
//! Once running, you can verify the server is operational:
//! ```bash
//! curl http://localhost:3000/ping
//! ```

mod server;
mod routes;
mod auth;
mod database;
mod services;
mod onchain_instance;
mod agent;
mod config;

use tracing_subscriber::{ layer::SubscriberExt, util::SubscriberInitExt };

/// Application entry point.
///
/// Initializes the tracing/logging system and starts the HTTP server.
/// This function will run indefinitely until the process is terminated.
///
/// # Logging Configuration
/// Sets up structured logging with the following features:
/// - Console output with timestamps
/// - JSON formatting for structured logs
/// - Configurable log levels (defaults to INFO)
///
/// # Server Lifecycle
/// 1. Initialize logging/tracing subscriber
/// 2. Start the HTTP server
/// 3. Run until process termination (Ctrl+C, SIGTERM, etc.)
///
/// # Error Handling
/// If the server fails to start, the process will exit with a panic.
/// In production environments, you may want to implement more graceful
/// error handling and recovery mechanisms.
#[tokio::main]
async fn main() {
    // Initialize the tracing subscriber for structured logging
    // This sets up console output with timestamps and proper formatting
    tracing_subscriber
        ::registry()
        .with(
            tracing_subscriber::fmt
                ::layer()
                .with_target(false) // Don't show module targets for cleaner output
                .compact() // Use compact formatting
        )
        .init();

    // Log application startup
    tracing::info!("🏁 Starting ICM Server...");
    tracing::info!("� Package: {} v{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    tracing::info!("🏗️  Build profile: {}", if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    });

    // Start the HTTP server - this will run indefinitely
    server::start().await;
}
