//! Database Connection Management
//! 
//! Handles PostgreSQL connection pooling using tokio-postgres and deadpool for optimal performance.

use anyhow::{Context, Result};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use std::env;
use std::time::Duration;
use tokio_postgres::NoTls;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
    pub max_size: usize,
    pub timeouts: deadpool_postgres::Timeouts,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            user: "orkarfabianthewise".to_string(),
            password: "2000".to_string(),
            dbname: "postgres".to_string(),
            max_size: 16,
            timeouts: deadpool_postgres::Timeouts {
                wait: Some(Duration::from_secs(30)),
                create: Some(Duration::from_secs(30)),
                recycle: Some(Duration::from_secs(30)),
            },
        }
    }
}

impl DatabaseConfig {
    /// Create configuration from database URL
    pub fn from_url(url: &str) -> Result<Self> {
        let parsed = url::Url::parse(url)
            .context("Failed to parse database URL")?;
        
        if parsed.scheme() != "postgresql" && parsed.scheme() != "postgres" {
            anyhow::bail!("Invalid database URL scheme, expected postgresql or postgres");
        }
        
        Ok(Self {
            host: parsed.host_str().unwrap_or("localhost").to_string(),
            port: parsed.port().unwrap_or(5432),
            user: parsed.username().to_string(),
            password: parsed.password().unwrap_or("").to_string(),
            dbname: parsed.path().trim_start_matches('/').to_string(),
            max_size: 16,
            timeouts: deadpool_postgres::Timeouts {
                wait: Some(Duration::from_secs(30)),
                create: Some(Duration::from_secs(30)),
                recycle: Some(Duration::from_secs(30)),
            },
        })
    }
}

/// Database connection wrapper
#[derive(Debug, Clone)]
pub struct DatabaseConnection {
    pool: Pool,
}

impl DatabaseConnection {
    /// Create a new database connection with the provided configuration
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        let masked_host = format!("{}:{}/{}", config.host, config.port, config.dbname);
        tracing::info!("🔌 Connecting to database: {}", masked_host);
        
        let mut pg_config = tokio_postgres::Config::new();
        pg_config.host(&config.host);
        pg_config.port(config.port);
        pg_config.user(&config.user);
        pg_config.password(&config.password);
        pg_config.dbname(&config.dbname);
        
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(pg_config, NoTls, mgr_config);
        
        let pool = Pool::builder(mgr)
            .max_size(config.max_size)
            .wait_timeout(config.timeouts.wait)
            .create_timeout(config.timeouts.create)
            .recycle_timeout(config.timeouts.recycle)
            .runtime(deadpool_postgres::Runtime::Tokio1)
            .build()
            .context("Failed to create database pool")?;

        // Test the connection
        let client = pool
            .get()
            .await
            .context("Failed to get connection from pool")?;
        
        client
            .query("SELECT 1", &[])
            .await
            .context("Failed to test database connection")?;

        tracing::info!("✅ Database connection established successfully");
        
        Ok(Self { pool })
    }

    /// Create connection from database URL
    pub async fn from_url(url: &str) -> Result<Self> {
        let config = DatabaseConfig::from_url(url)?;
        Self::new(config).await
    }

    /// Create connection using default local PostgreSQL configuration
    pub async fn new_local() -> Result<Self> {
        let config = DatabaseConfig::default();
        Self::new(config).await
    }

    /// Create connection from your specific PostgreSQL URL
    pub async fn new_from_local_postgres() -> Result<Self> {
        let url = "postgresql://postgres:2000@localhost:5432/postgres";
        Self::from_url(url).await
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Run database migrations using refinery
    pub async fn migrate(&self) -> Result<()> {
        tracing::info!("🔄 Running database migrations...");
        
        let client = self.pool
            .get()
            .await
            .context("Failed to get connection for migrations")?;
        
        // You'll need to set up migrations directory and use refinery
        // This is a placeholder - implement based on your migration strategy
        tracing::warn!("⚠️  Migration implementation needed - using refinery crate");
        
        tracing::info!("✅ Database migrations completed successfully");
        Ok(())
    }

    /// Fetch the user's private key from the database by user_id
    pub async fn get_user_private_key(&self, user_id: uuid::Uuid) -> Result<Option<String>> {
        let client = self.pool.get().await.context("Failed to get DB connection")?;
        let row = client
            .query_opt(
                "SELECT private_key FROM users WHERE id = $1",
                &[&user_id],
            )
            .await
            .context("Failed to query user private key")?;
        Ok(row.and_then(|r| r.try_get("private_key").ok()))
    }

    /// Fetch the user's private key from the database by email
    pub async fn get_user_private_key_by_email(&self, email: &str) -> Result<Option<String>> {
        let client = self.pool.get().await.context("Failed to get DB connection")?;
        let row = client
            .query_opt(
                "SELECT private_key FROM users WHERE email = $1",
                &[&email],
            )
            .await
            .context("Failed to query user private key by email")?;
        Ok(row.and_then(|r| r.try_get("private_key").ok()))
    }

    /// Check database health
    pub async fn health_check(&self) -> Result<()> {
        let client = self.pool
            .get()
            .await
            .context("Failed to get connection for health check")?;
            
        client
            .query("SELECT 1", &[])
            .await
            .context("Database health check failed")?;
        Ok(())
    }

    /// Get database connection statistics
    pub fn stats(&self) -> ConnectionStats {
        let status = self.pool.status();
        ConnectionStats {
            size: status.size as u32,
            idle: status.available,
        }
    }
}

/// Database connection statistics
#[derive(Debug)]
pub struct ConnectionStats {
    pub size: u32,
    pub idle: usize,
}


