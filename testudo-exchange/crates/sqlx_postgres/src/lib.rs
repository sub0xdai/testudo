use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

// Export the repositories module
pub mod repositories;

// Re-export commonly used components
pub use repositories::{
    CreateExchangeAccountRequest, ExchangeAccountFilter, ExchangeAccountRepository,
    ExchangeAccountSummary, ExchangeAccountWithCredentials, PostgresExchangeAccountRepository,
    RepositoryError, UpdateExchangeAccountRequest,
};

pub struct PostgresDb {
    pool: sqlx::Pool<sqlx::Postgres>,
    /// JNL-12 FR-7: Dedicated pool for journal analytics (read-heavy, long-running queries).
    analytics_pool: sqlx::Pool<sqlx::Postgres>,
}

impl PostgresDb {
    /// Create a new PostgresDb with optimized connection pooling.
    ///
    /// Configuration (see FR-2.2 in 006-performance-overhaul):
    /// - max_connections: 50 (configurable via DB_MAX_CONNECTIONS env var)
    /// - acquire_timeout: 500ms to prevent indefinite blocking
    pub async fn new() -> Result<Self, sqlx::Error> {
        let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let max_connections: u32 = std::env::var("DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50);

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_millis(500))
            .connect(&db_url)
            .await?;

        // JNL-12 FR-7: Separate analytics pool for journal read queries.
        // Prevents long-running analytical queries from starving OLTP connections.
        let analytics_max: u32 = std::env::var("DB_ANALYTICS_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let analytics_pool = PgPoolOptions::new()
            .max_connections(analytics_max)
            .acquire_timeout(Duration::from_secs(5)) // analytics can wait longer
            .connect(&db_url)
            .await?;

        println!(
            "Connected to Postgres - {} (max_connections: {}, analytics: {})",
            db_url, max_connections, analytics_max
        );

        // Run the table creation query if it doesn't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS trades (
                trade_id BIGINT PRIMARY KEY,
                market VARCHAR NOT NULL,
                price NUMERIC NOT NULL,
                quantity NUMERIC NOT NULL,
                user_id VARCHAR NOT NULL,
                other_user_id VARCHAR NOT NULL,
                order_id VARCHAR NOT NULL,
                timestamp BIGINT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool, analytics_pool })
    }

    pub fn get_pg_connection(&self) -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
        Ok(self.pool.clone())
    }

    /// Gets the underlying connection pool for use with repositories
    pub fn pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        &self.pool
    }

    /// JNL-12 FR-7: Gets the analytics pool for journal read queries.
    pub fn analytics_pool(&self) -> &sqlx::Pool<sqlx::Postgres> {
        &self.analytics_pool
    }
}
