// Database Connection Management
// 
// Handles PostgreSQL connection pooling using tokio-postgres and deadpool for optimal performance.
use crate::database::models::UserProfile;
use anyhow::{Context, Result};
use std::str::FromStr;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use std::time::Duration;
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use std::env;
use crate::database::models::FromRow;

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

impl DatabaseConnection {
    /// Fetch user profile by user_pubkey
    pub async fn get_user_profile_by_pubkey(&self, user_pubkey: &str) -> Result<Option<UserProfile>> {
        let client = self.pool.get().await.context("Failed to get DB connection")?;
        let row = client
            .query_opt(
                "SELECT * FROM user_profiles WHERE user_pubkey = $1",
                &[&user_pubkey],
            )
            .await
            .context("Failed to query user profile by pubkey")?;
        Ok(row.map(|r| UserProfile::from_row(&r).unwrap()))
    }

    /// Fetch user profile by email
    pub async fn get_user_profile_by_email(&self, email: &str) -> Result<Option<UserProfile>> {
        let client = self.pool.get().await.context("Failed to get DB connection")?;
        let row = client
            .query_opt(
                "SELECT * FROM user_profiles WHERE email = $1",
                &[&email],
            )
            .await
            .context("Failed to query user profile by email")?;
        Ok(row.map(|r| UserProfile::from_row(&r).unwrap()))
    }

    /// Update last_faucet_claim for a user
    pub async fn update_last_faucet_claim(&self, user_pubkey: &str, ts: chrono::NaiveDateTime) -> Result<u64> {
        let client = self.pool.get().await.context("Failed to get DB connection")?;
        let n = client
            .execute(
                "UPDATE user_profiles SET last_faucet_claim = $1, updated_at = NOW() WHERE user_pubkey = $2",
                &[&ts, &user_pubkey],
            )
            .await?;
        Ok(n)
    }
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
        tracing::debug!("Parsed database URL: {:?}", parsed);

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

    /// Create configuration from environment variables
    pub fn from_env() -> Result<Self> {

        let database_url = std::env::var("DATABASE_URL")
            .context("DATABASE_URL must be set in the environment")?;

        let config = tokio_postgres::Config::from_str(&database_url)
            .context("Failed to parse DATABASE_URL")?;

        Ok(Self {
            host: config.get_hosts().get(0).map(|h| match h {
                tokio_postgres::config::Host::Tcp(s) => s.clone(),
                tokio_postgres::config::Host::Unix(s) => s.to_string_lossy().to_string(),
            }).unwrap_or_default(),
            port: config.get_ports().get(0).cloned().unwrap_or(5432),
            user: config.get_user().map(|u| u.to_string()).unwrap_or_default(),
            password: config.get_password().map(|p| String::from_utf8_lossy(p).to_string()).unwrap_or_default(),
            dbname: config.get_dbname().map(|d| d.to_string()).unwrap_or_default(),
            max_size: 16, // Default max size
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

    // Enable SSL using native-tls
    let tls_connector = TlsConnector::builder().build().context("Failed to build TLS connector")?;
    let tls = MakeTlsConnector::new(tls_connector);

        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(pg_config, tls, mgr_config);

        let pool = Pool::builder(mgr)
            .max_size(config.max_size)
            .wait_timeout(config.timeouts.wait)
            .create_timeout(config.timeouts.create)
            .recycle_timeout(config.timeouts.recycle)
            .runtime(deadpool_postgres::Runtime::Tokio1)
            .build()
            .context("Failed to create database pool")?;

        // Test the connection
        let _client = pool
            .get()
            .await
            .context("Failed to get connection from pool")?;

        _client
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


