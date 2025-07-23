//! Configuration module for environment variables and application settings

use std::env;
use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;

/// Global application configuration loaded from environment variables
pub static CONFIG: Lazy<Config> = Lazy::new(|| {
    Config::from_env().expect("Failed to load configuration from environment")
});

#[derive(Debug, Clone)]
pub struct Config {
    /// OpenAI API key for AI trading decisions
    pub openai_api_key: String,
    
    /// Jupiter API base URL
    pub jupiter_api_url: String,
    
    /// Database configuration
    pub database: DatabaseConfig,
    
    /// Server configuration
    pub server: ServerConfig,
    
    /// Trading agent configuration
    pub trading: TradingConfig,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct TradingConfig {
    /// Maximum concurrent trades
    pub max_concurrent_trades: usize,
    /// Default quote cache TTL in seconds
    pub quote_cache_ttl: u64,
    /// Risk assessment threshold (0.0 to 1.0)
    pub risk_threshold: f64,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            openai_api_key: env::var("OPENAI_API_KEY")
                .map_err(|_| anyhow!("OPENAI_API_KEY environment variable is required"))?,
                
            jupiter_api_url: env::var("JUPITER_API_URL")
                .unwrap_or_else(|_| "https://quote-api.jup.ag/v6".to_string()),
                
            database: DatabaseConfig {
                url: env::var("DATABASE_URL")
                    .unwrap_or_else(|_| "postgres://user:password@localhost/icm_db".to_string()),
                max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
            },
            
            server: ServerConfig {
                host: env::var("SERVER_HOST")
                    .unwrap_or_else(|_| "127.0.0.1".to_string()),
                port: env::var("SERVER_PORT")
                    .unwrap_or_else(|_| "3000".to_string())
                    .parse()
                    .unwrap_or(3000),
            },
            
            trading: TradingConfig {
                max_concurrent_trades: env::var("MAX_CONCURRENT_TRADES")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .unwrap_or(5),
                quote_cache_ttl: env::var("QUOTE_CACHE_TTL")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
                risk_threshold: env::var("RISK_THRESHOLD")
                    .unwrap_or_else(|_| "0.8".to_string())
                    .parse()
                    .unwrap_or(0.8),
            },
        })
    }
}
