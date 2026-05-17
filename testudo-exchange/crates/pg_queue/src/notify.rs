use crate::errors::Result;
use sqlx::PgPool;

/// Service for sending PostgreSQL NOTIFY messages
#[derive(Clone)]
pub struct NotifyService {
    pool: PgPool,
}

impl NotifyService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Send a notification to a channel
    /// Note: PostgreSQL NOTIFY has an 8KB payload limit
    pub async fn notify(&self, channel: &str, payload: &str) -> Result<()> {
        sqlx::query("SELECT pg_notify($1, $2)")
            .bind(channel)
            .bind(payload)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Publish to a trade stream channel (trade.{symbol})
    pub async fn publish_trade(&self, symbol: &str, payload: &str) -> Result<()> {
        let channel = format!("trade.{}", symbol);
        self.notify(&channel, payload).await
    }

    /// Publish to a depth stream channel (depth.{symbol})
    pub async fn publish_depth(&self, symbol: &str, payload: &str) -> Result<()> {
        let channel = format!("depth.{}", symbol);
        self.notify(&channel, payload).await
    }

    /// Publish a response to a request-specific channel
    pub async fn publish_response(&self, request_id: &str, payload: &str) -> Result<()> {
        self.notify(request_id, payload).await
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_channel_formatting() {
        // These are unit tests that don't need a database
        let trade_channel = format!("trade.{}", "BTC_USDC");
        assert_eq!(trade_channel, "trade.BTC_USDC");

        let depth_channel = format!("depth.{}", "SOL_USDC");
        assert_eq!(depth_channel, "depth.SOL_USDC");
    }
}
