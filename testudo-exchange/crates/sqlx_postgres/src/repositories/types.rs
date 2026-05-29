//! Request and response types for exchange account repository operations
//!
//! This module defines the data transfer objects used for repository operations,
//! separating business logic from database representation.

// @anchor exchange:sqlx_postgres:types
// @tags infra

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Request to create a new exchange account
/// Contains plaintext credentials that will be encrypted by the repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateExchangeAccountRequest {
    pub user_id: Uuid,
    pub exchange_name: String,
    pub api_key: String,    // Plaintext input - will be encrypted
    pub api_secret: String, // Plaintext input - will be encrypted
    pub permissions: serde_json::Value,
}

/// Request to update an existing exchange account
/// Only includes fields that can be updated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateExchangeAccountRequest {
    pub id: Uuid,
    pub api_key: Option<String>,    // Optional - only update if provided
    pub api_secret: Option<String>, // Optional - only update if provided
    pub permissions: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

/// Response containing decrypted exchange account credentials
/// Used when the application needs access to actual API keys
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeAccountWithCredentials {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange_name: String,
    pub api_key: String,    // Decrypted plaintext
    pub api_secret: String, // Decrypted plaintext
    pub permissions: serde_json::Value,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Summary response without sensitive credentials
/// Used for listing accounts or when credentials are not needed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeAccountSummary {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange_name: String,
    pub exchange_display_name: String,
    pub permissions: serde_json::Value,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub has_credentials: bool, // Indicates if valid credentials are stored
}

/// Database row representation for mapping from SQLx queries
/// Maps directly to the database schema
#[derive(Debug, sqlx::FromRow)]
pub struct ExchangeAccountRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange_name: String,
    pub api_key_encrypted: Vec<u8>,
    pub api_secret_encrypted: Vec<u8>,
    pub permissions: serde_json::Value,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Query filters for listing exchange accounts
#[derive(Debug, Clone, Default)]
pub struct ExchangeAccountFilter {
    pub user_id: Option<Uuid>,
    pub exchange_name: Option<String>,
    pub is_active: Option<bool>,
    pub created_after: Option<chrono::DateTime<chrono::Utc>>,
    pub created_before: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl ExchangeAccountFilter {
    pub fn for_user(user_id: Uuid) -> Self {
        Self {
            user_id: Some(user_id),
            ..Default::default()
        }
    }

    pub fn for_user_and_exchange(user_id: Uuid, exchange_name: String) -> Self {
        Self {
            user_id: Some(user_id),
            exchange_name: Some(exchange_name),
            ..Default::default()
        }
    }

    pub fn active_only(mut self) -> Self {
        self.is_active = Some(true);
        self
    }

    pub fn with_limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }
}

impl CreateExchangeAccountRequest {
    /// Validates the request and normalizes the exchange name
    pub fn validate_and_normalize(&mut self) -> Result<(), String> {
        // Validate user ID
        if self.user_id.is_nil() {
            return Err("User ID cannot be nil".to_string());
        }

        // Validate exchange name
        if self.exchange_name.trim().is_empty() {
            return Err("Exchange name cannot be empty".to_string());
        }
        if self.exchange_name.len() > 50 {
            return Err("Exchange name must be between 1 and 50 characters".to_string());
        }

        // Validate API credentials
        if self.api_key.is_empty() {
            return Err("API key cannot be empty".to_string());
        }
        if self.api_secret.is_empty() {
            return Err("API secret cannot be empty".to_string());
        }

        // Validate permissions
        if !self.permissions.is_object() {
            return Err("Permissions must be a JSON object".to_string());
        }

        // Normalize exchange name to lowercase
        self.exchange_name = self.exchange_name.to_lowercase().trim().to_string();

        Ok(())
    }
}

impl UpdateExchangeAccountRequest {
    /// Validates the update request
    pub fn validate_fields(&self) -> Result<(), String> {
        // Validate API key if provided
        if let Some(ref api_key) = self.api_key {
            if api_key.is_empty() {
                return Err("API key cannot be empty".to_string());
            }
        }

        // Validate API secret if provided
        if let Some(ref api_secret) = self.api_secret {
            if api_secret.is_empty() {
                return Err("API secret cannot be empty".to_string());
            }
        }

        // Validate permissions if provided
        if let Some(ref permissions) = self.permissions {
            if !permissions.is_object() {
                return Err("Permissions must be a JSON object".to_string());
            }
        }

        Ok(())
    }

    /// Checks if the request has any fields to update
    pub fn has_updates(&self) -> bool {
        self.api_key.is_some()
            || self.api_secret.is_some()
            || self.permissions.is_some()
            || self.is_active.is_some()
    }
}

impl From<ExchangeAccountRow> for ExchangeAccountSummary {
    fn from(row: ExchangeAccountRow) -> Self {
        let exchange_display_name = match row.exchange_name.as_str() {
            "coinbase_pro" => "Coinbase Pro".to_string(),
            name => {
                let mut chars = name.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        };

        Self {
            id: row.id,
            user_id: row.user_id,
            exchange_name: row.exchange_name,
            exchange_display_name,
            permissions: row.permissions,
            is_active: row.is_active,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
            has_credentials: !row.api_key_encrypted.is_empty()
                && !row.api_secret_encrypted.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn mock_user_id() -> Uuid {
        Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
    }

    fn mock_permissions() -> serde_json::Value {
        json!({
            "spot_trading": true,
            "futures_trading": false,
            "withdrawals": false
        })
    }

    #[test]
    fn should_validate_create_request() {
        let mut request = CreateExchangeAccountRequest {
            user_id: mock_user_id(),
            exchange_name: "  BINANCE  ".to_string(),
            api_key: "test_api_key".to_string(),
            api_secret: "test_api_secret".to_string(),
            permissions: mock_permissions(),
        };

        assert!(request.validate_and_normalize().is_ok());
        assert_eq!(request.exchange_name, "binance"); // Should be normalized
    }

    #[test]
    fn should_reject_invalid_create_request() {
        // Test nil user ID
        let mut request = CreateExchangeAccountRequest {
            user_id: Uuid::nil(),
            exchange_name: "binance".to_string(),
            api_key: "test_api_key".to_string(),
            api_secret: "test_api_secret".to_string(),
            permissions: mock_permissions(),
        };
        assert!(request.validate_and_normalize().is_err());

        // Test empty exchange name
        let mut request = CreateExchangeAccountRequest {
            user_id: mock_user_id(),
            exchange_name: "".to_string(),
            api_key: "test_api_key".to_string(),
            api_secret: "test_api_secret".to_string(),
            permissions: mock_permissions(),
        };
        assert!(request.validate_and_normalize().is_err());

        // Test empty API key
        let mut request = CreateExchangeAccountRequest {
            user_id: mock_user_id(),
            exchange_name: "binance".to_string(),
            api_key: "".to_string(),
            api_secret: "test_api_secret".to_string(),
            permissions: mock_permissions(),
        };
        assert!(request.validate_and_normalize().is_err());

        // Test invalid permissions
        let mut request = CreateExchangeAccountRequest {
            user_id: mock_user_id(),
            exchange_name: "binance".to_string(),
            api_key: "test_api_key".to_string(),
            api_secret: "test_api_secret".to_string(),
            permissions: json!("not_an_object"),
        };
        assert!(request.validate_and_normalize().is_err());
    }

    #[test]
    fn should_validate_update_request() {
        let request = UpdateExchangeAccountRequest {
            id: mock_user_id(),
            api_key: Some("new_api_key".to_string()),
            api_secret: None,
            permissions: Some(mock_permissions()),
            is_active: Some(false),
        };

        assert!(request.validate_fields().is_ok());
        assert!(request.has_updates());
    }

    #[test]
    fn should_reject_invalid_update_request() {
        // Test empty API key
        let request = UpdateExchangeAccountRequest {
            id: mock_user_id(),
            api_key: Some("".to_string()),
            api_secret: None,
            permissions: None,
            is_active: None,
        };
        assert!(request.validate_fields().is_err());

        // Test invalid permissions
        let request = UpdateExchangeAccountRequest {
            id: mock_user_id(),
            api_key: None,
            api_secret: None,
            permissions: Some(json!("not_an_object")),
            is_active: None,
        };
        assert!(request.validate_fields().is_err());
    }

    #[test]
    fn should_detect_no_updates() {
        let request = UpdateExchangeAccountRequest {
            id: mock_user_id(),
            api_key: None,
            api_secret: None,
            permissions: None,
            is_active: None,
        };

        assert!(!request.has_updates());
    }

    #[test]
    fn should_create_filters() {
        let user_id = mock_user_id();

        // Test for_user filter
        let filter = ExchangeAccountFilter::for_user(user_id);
        assert_eq!(filter.user_id, Some(user_id));
        assert_eq!(filter.exchange_name, None);

        // Test for_user_and_exchange filter
        let filter = ExchangeAccountFilter::for_user_and_exchange(user_id, "binance".to_string());
        assert_eq!(filter.user_id, Some(user_id));
        assert_eq!(filter.exchange_name, Some("binance".to_string()));

        // Test chaining
        let filter = ExchangeAccountFilter::for_user(user_id)
            .active_only()
            .with_limit(10)
            .with_offset(5);

        assert_eq!(filter.user_id, Some(user_id));
        assert_eq!(filter.is_active, Some(true));
        assert_eq!(filter.limit, Some(10));
        assert_eq!(filter.offset, Some(5));
    }

    #[test]
    fn should_convert_row_to_summary() {
        let row = ExchangeAccountRow {
            id: mock_user_id(),
            user_id: mock_user_id(),
            exchange_name: "binance".to_string(),
            api_key_encrypted: b"encrypted_key".to_vec(),
            api_secret_encrypted: b"encrypted_secret".to_vec(),
            permissions: mock_permissions(),
            is_active: true,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        };

        let summary = ExchangeAccountSummary::from(row);
        assert_eq!(summary.exchange_name, "binance");
        assert_eq!(summary.exchange_display_name, "Binance");
        assert!(summary.has_credentials);

        // Test special case for coinbase_pro
        let row = ExchangeAccountRow {
            id: mock_user_id(),
            user_id: mock_user_id(),
            exchange_name: "coinbase_pro".to_string(),
            api_key_encrypted: b"encrypted_key".to_vec(),
            api_secret_encrypted: b"encrypted_secret".to_vec(),
            permissions: mock_permissions(),
            is_active: true,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        };

        let summary = ExchangeAccountSummary::from(row);
        assert_eq!(summary.exchange_display_name, "Coinbase Pro");
    }

    #[test]
    fn should_detect_missing_credentials() {
        let row = ExchangeAccountRow {
            id: mock_user_id(),
            user_id: mock_user_id(),
            exchange_name: "binance".to_string(),
            api_key_encrypted: Vec::new(),    // Empty
            api_secret_encrypted: Vec::new(), // Empty
            permissions: mock_permissions(),
            is_active: true,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        };

        let summary = ExchangeAccountSummary::from(row);
        assert!(!summary.has_credentials);
    }
}
