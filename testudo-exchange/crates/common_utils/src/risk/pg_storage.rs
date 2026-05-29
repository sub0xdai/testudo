//! Risk Config Storage (PostgreSQL)
//!
//! Persistence layer for user risk configurations using PostgreSQL.
//! Provides save/load/delete operations with automatic fallback to defaults.

// @anchor exchange:common_utils:pg_storage
// @tags infra

use super::config::{RiskConfig, RiskConfigError};
use crate::services::pg_cache::{CacheError, PgCacheService};
use std::time::Duration;
use uuid::Uuid;

/// Cache key prefix for risk configs
const RISK_CONFIG_PREFIX: &str = "risk:config";

/// TTL for risk configs (24 hours - configs are semi-permanent)
const RISK_CONFIG_TTL: Duration = Duration::from_secs(86400);

/// Storage errors for risk configuration
#[derive(Debug, thiserror::Error)]
pub enum PgRiskStorageError {
    #[error("Cache error: {0}")]
    CacheError(#[from] CacheError),

    #[error("Config validation error: {0}")]
    ValidationError(#[from] RiskConfigError),

    #[error("User not found: {0}")]
    UserNotFound(Uuid),
}

/// Risk configuration storage service using PostgreSQL
#[derive(Clone)]
pub struct PgRiskConfigStorage {
    cache: PgCacheService,
}

impl PgRiskConfigStorage {
    /// Create a new risk config storage from a PostgreSQL cache service
    pub fn new(cache: PgCacheService) -> Self {
        Self { cache }
    }

    /// Build the cache key for a user's risk config
    fn config_key(user_id: Uuid) -> String {
        format!("{}:{}", RISK_CONFIG_PREFIX, user_id)
    }

    /// Save a user's risk configuration
    ///
    /// Validates the config before saving. Returns an error if validation fails.
    pub async fn save(&self, config: &RiskConfig) -> Result<(), PgRiskStorageError> {
        // Validate config before saving
        config.validate()?;

        let user_id = config.user_id.ok_or_else(|| {
            RiskConfigError::InvalidRiskPercent("user_id is required for storage".to_string())
        })?;

        let key = Self::config_key(user_id);
        self.cache.set(&key, config, RISK_CONFIG_TTL).await?;

        Ok(())
    }

    /// Load a user's risk configuration
    ///
    /// Returns the user's saved config, or None if not found.
    pub async fn load(&self, user_id: Uuid) -> Result<Option<RiskConfig>, PgRiskStorageError> {
        let key = Self::config_key(user_id);

        match self.cache.get::<RiskConfig>(&key).await {
            Ok(config) => Ok(Some(config)),
            Err(CacheError::CacheMiss(_)) => Ok(None),
            Err(e) => Err(PgRiskStorageError::CacheError(e)),
        }
    }

    /// Load a user's risk configuration with fallback to default
    ///
    /// Returns the user's saved config if found, otherwise returns the default config
    /// with the user_id set.
    pub async fn load_or_default(&self, user_id: Uuid) -> Result<RiskConfig, PgRiskStorageError> {
        match self.load(user_id).await? {
            Some(config) => Ok(config),
            None => Ok(RiskConfig::default().with_user_id(user_id)),
        }
    }

    /// Delete a user's risk configuration
    ///
    /// Returns Ok even if the config didn't exist.
    pub async fn delete(&self, user_id: Uuid) -> Result<(), PgRiskStorageError> {
        let key = Self::config_key(user_id);
        self.cache.delete(&key).await?;
        Ok(())
    }

    /// Check if a user has a saved risk configuration
    pub async fn exists(&self, user_id: Uuid) -> Result<bool, PgRiskStorageError> {
        Ok(self.load(user_id).await?.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_config_key_format() {
        let user_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = PgRiskConfigStorage::config_key(user_id);
        assert_eq!(key, "risk:config:550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn test_risk_config_serialization() {
        let user_id = Uuid::new_v4();
        let config = RiskConfig::new()
            .with_user_id(user_id)
            .with_account_risk_percent(dec!(2.5))
            .with_max_risk_amount(dec!(100))
            .with_max_position_size(dec!(0.5));

        // Verify serialization works
        let json = serde_json::to_string(&config).expect("Should serialize");
        let deserialized: RiskConfig = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(deserialized.user_id, Some(user_id));
        assert_eq!(deserialized.account_risk_percent, dec!(2.5));
        assert_eq!(deserialized.max_risk_amount, Some(dec!(100)));
        assert_eq!(deserialized.max_position_size, Some(dec!(0.5)));
    }
}
