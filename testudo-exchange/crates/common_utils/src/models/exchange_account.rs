// @anchor exchange:common_utils:exchange_account
// @tags infra

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;
use validator::Validate;

/// Exchange Account domain model - stores CEX API credentials securely
/// Following Single Responsibility Principle: only handles exchange account data
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ExchangeAccount {
    pub id: Uuid,
    #[validate(custom(function = "validate_user_id"))]
    pub user_id: Uuid,
    // Note: exchange_name validation is handled in factory, not here, to support DI
    pub exchange_name: String,
    #[serde(skip_serializing, default = "default_encrypted_field")]
    pub encrypted_api_key: Vec<u8>,
    #[serde(skip_serializing, default = "default_encrypted_field")]
    pub encrypted_secret: Vec<u8>,
    pub permissions: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub auth_mode: String,
    pub wallet_address: Option<String>,
}

// Custom Display implementation that masks sensitive data
impl fmt::Display for ExchangeAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExchangeAccount {{ id: {}, user_id: {}, exchange: {}, active: {}, created: {} }}",
            self.id,
            self.user_id,
            self.exchange_name,
            self.is_active,
            self.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        )
    }
}

/// Exchange validator abstraction following Interface Segregation Principle
pub trait ExchangeValidator: Send + Sync {
    fn validate_exchange_name(&self, name: &str) -> Result<(), ExchangeAccountError>;
    fn validate_permissions(
        &self,
        permissions: &serde_json::Value,
    ) -> Result<(), ExchangeAccountError>;
}

/// Standard exchange validator implementation
pub struct StandardExchangeValidator;

impl ExchangeValidator for StandardExchangeValidator {
    fn validate_exchange_name(&self, name: &str) -> Result<(), ExchangeAccountError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(ExchangeAccountError::InvalidExchange(
                "Exchange name cannot be empty".to_string(),
            ));
        }

        // EXT-15: Accept any non-empty name — CCXT sidecar validates dynamically
        Ok(())
    }

    fn validate_permissions(
        &self,
        permissions: &serde_json::Value,
    ) -> Result<(), ExchangeAccountError> {
        if !permissions.is_object() {
            return Err(ExchangeAccountError::InvalidPermissions(
                "Permissions must be a JSON object".to_string(),
            ));
        }
        Ok(())
    }
}

/// Exchange Account factory following Single Responsibility Principle
pub struct ExchangeAccountFactory<V: ExchangeValidator> {
    validator: V,
}

impl<V: ExchangeValidator> ExchangeAccountFactory<V> {
    pub fn new(validator: V) -> Self {
        Self { validator }
    }

    pub fn create_exchange_account(
        &self,
        user_id: Uuid,
        exchange_name: &str,
        encrypted_api_key: Vec<u8>,
        encrypted_secret: Vec<u8>,
        permissions: serde_json::Value,
    ) -> Result<ExchangeAccount, ExchangeAccountError> {
        // Validate inputs using injected validator
        if user_id.is_nil() {
            return Err(ExchangeAccountError::InvalidUserId(
                "User ID cannot be nil".to_string(),
            ));
        }

        self.validator.validate_exchange_name(exchange_name)?;
        self.validator.validate_permissions(&permissions)?;

        if encrypted_api_key.is_empty() {
            return Err(ExchangeAccountError::InvalidCredentials(
                "API key cannot be empty".to_string(),
            ));
        }

        if encrypted_secret.is_empty() {
            return Err(ExchangeAccountError::InvalidCredentials(
                "API secret cannot be empty".to_string(),
            ));
        }

        let now = Utc::now();
        let account = ExchangeAccount {
            id: Uuid::new_v4(),
            user_id,
            exchange_name: exchange_name.to_lowercase(), // Normalize to lowercase
            encrypted_api_key,
            encrypted_secret,
            permissions,
            is_active: true,
            created_at: now,
            last_used_at: None,
            auth_mode: "api_key".to_string(),
            wallet_address: None,
        };

        // Final validation using validator crate (only validates user_id now)
        account
            .validate()
            .map_err(|e| ExchangeAccountError::ValidationFailed(format!("{:?}", e)))?;

        Ok(account)
    }
}

/// HIST-03: Normalize an exchange name to its canonical form for storage/index keys.
/// Single source of truth; prevents casing drift from defeating the partial unique
/// index `idx_unique_import_fill(user_id, exchange, exchange_fill_id)`.
pub fn canonical_exchange_name(name: &str) -> String {
    name.trim().to_lowercase()
}

impl ExchangeAccount {
    /// Updates the last_used_at timestamp to current time
    pub fn update_last_used(&mut self) {
        self.last_used_at = Some(Utc::now());
    }

    /// Deactivates the exchange account
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Activates the exchange account
    pub fn activate(&mut self) {
        self.is_active = true;
    }

    /// Updates permissions (validates first)
    pub fn update_permissions<V: ExchangeValidator>(
        &mut self,
        permissions: serde_json::Value,
        validator: &V,
    ) -> Result<(), ExchangeAccountError> {
        validator.validate_permissions(&permissions)?;
        self.permissions = permissions;
        Ok(())
    }

    /// Checks if account has specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        if let Some(perms) = self.permissions.as_object() {
            perms
                .get(permission)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Gets the exchange display name (capitalized)
    pub fn exchange_display_name(&self) -> String {
        match self.exchange_name.as_str() {
            "coinbase_pro" => "Coinbase Pro".to_string(),
            name => {
                let mut chars = name.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExchangeAccountError {
    #[error("Invalid exchange: {0}")]
    InvalidExchange(String),
    #[error("Unsupported exchange: {0}")]
    UnsupportedExchange(String),
    #[error("Invalid user ID: {0}")]
    InvalidUserId(String),
    #[error("Invalid credentials: {0}")]
    InvalidCredentials(String),
    #[error("Invalid permissions: {0}")]
    InvalidPermissions(String),
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
}

// Validation functions for the validator derive
fn validate_user_id(user_id: &Uuid) -> Result<(), validator::ValidationError> {
    if user_id.is_nil() {
        return Err(validator::ValidationError::new("user_id_nil"));
    }
    Ok(())
}

// Default value for encrypted fields during deserialization
fn default_encrypted_field() -> Vec<u8> {
    Vec::new()
}

// Type aliases for common use cases
pub type StandardExchangeAccountFactory = ExchangeAccountFactory<StandardExchangeValidator>;

impl Default for StandardExchangeAccountFactory {
    fn default() -> Self {
        Self::new(StandardExchangeValidator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Mock data for testing
    fn mock_user_id() -> Uuid {
        Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
    }

    fn mock_encrypted_key() -> Vec<u8> {
        b"encrypted_api_key_data".to_vec()
    }

    fn mock_encrypted_secret() -> Vec<u8> {
        b"encrypted_secret_data".to_vec()
    }

    fn mock_permissions() -> serde_json::Value {
        json!({
            "spot_trading": true,
            "futures_trading": false,
            "withdrawals": false
        })
    }

    // RED phase tests - these should fail first, then drive implementation

    #[test]
    fn should_create_valid_exchange_account() {
        let factory = StandardExchangeAccountFactory::default();
        let account = factory
            .create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            )
            .unwrap();

        assert_eq!(account.user_id, mock_user_id());
        assert_eq!(account.exchange_name, "binance");
        assert_eq!(account.encrypted_api_key, mock_encrypted_key());
        assert_eq!(account.encrypted_secret, mock_encrypted_secret());
        assert!(account.is_active);
        assert!(account.last_used_at.is_none());
        assert!(!account.id.is_nil());
    }

    #[test]
    fn should_reject_empty_exchange_names() {
        let factory = StandardExchangeAccountFactory::default();

        let invalid_exchanges = vec!["", "   "];

        for exchange in invalid_exchanges {
            let result = factory.create_exchange_account(
                mock_user_id(),
                exchange,
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            );
            assert!(
                result.is_err(),
                "Exchange '{}' should be rejected",
                exchange
            );
        }
    }

    #[test]
    fn should_accept_any_valid_exchange_name() {
        let factory = StandardExchangeAccountFactory::default();

        // EXT-15: CCXT sidecar validates dynamically — accept any non-empty name
        let exchanges = vec![
            "binance",
            "coinbase",
            "kraken",
            "BINANCE",      // Should normalize to lowercase
            "Coinbase_Pro", // Should normalize to lowercase
            "woo",          // Previously blocked by hardcoded list
            "deribit",
            "gateio",
        ];

        for exchange in exchanges {
            let result = factory.create_exchange_account(
                mock_user_id(),
                exchange,
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            );
            assert!(result.is_ok(), "Exchange '{}' should be accepted", exchange);
        }
    }

    #[test]
    fn should_reject_nil_user_id() {
        let factory = StandardExchangeAccountFactory::default();

        let result = factory.create_exchange_account(
            Uuid::nil(),
            "binance",
            mock_encrypted_key(),
            mock_encrypted_secret(),
            mock_permissions(),
        );

        assert!(result.is_err());
        if let Err(ExchangeAccountError::InvalidUserId(_)) = result {
            // Expected error type
        } else {
            panic!("Expected InvalidUserId error");
        }
    }

    #[test]
    fn should_reject_empty_credentials() {
        let factory = StandardExchangeAccountFactory::default();

        // Test empty API key
        let result1 = factory.create_exchange_account(
            mock_user_id(),
            "binance",
            Vec::new(),
            mock_encrypted_secret(),
            mock_permissions(),
        );
        assert!(result1.is_err());

        // Test empty secret
        let result2 = factory.create_exchange_account(
            mock_user_id(),
            "binance",
            mock_encrypted_key(),
            Vec::new(),
            mock_permissions(),
        );
        assert!(result2.is_err());
    }

    #[test]
    fn should_reject_invalid_permissions() {
        let factory = StandardExchangeAccountFactory::default();

        let invalid_permissions = vec![json!("not_an_object"), json!(123), json!(true)];

        for permissions in invalid_permissions {
            let result = factory.create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                permissions,
            );
            assert!(result.is_err(), "Invalid permissions should be rejected");
        }
    }

    #[test]
    fn should_not_expose_sensitive_data_in_serialization() {
        let factory = StandardExchangeAccountFactory::default();
        let account = factory
            .create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            )
            .unwrap();

        let serialized = serde_json::to_string(&account).expect("Should serialize");

        // Should not contain encrypted data
        assert!(!serialized.contains("encrypted_api_key_data"));
        assert!(!serialized.contains("encrypted_secret_data"));

        // Should contain non-sensitive data
        assert!(serialized.contains("binance"));
        assert!(serialized.contains(&account.id.to_string()));
    }

    #[test]
    fn should_not_expose_sensitive_data_in_display() {
        let factory = StandardExchangeAccountFactory::default();
        let account = factory
            .create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            )
            .unwrap();

        let display_string = format!("{}", account);

        // Should not contain encrypted data
        assert!(!display_string.contains("encrypted_api_key_data"));
        assert!(!display_string.contains("encrypted_secret_data"));

        // Should contain safe data
        assert!(display_string.contains("binance"));
        assert!(display_string.contains(&account.id.to_string()));
    }

    #[test]
    fn should_update_last_used_timestamp() {
        let factory = StandardExchangeAccountFactory::default();
        let mut account = factory
            .create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            )
            .unwrap();

        assert!(account.last_used_at.is_none());

        account.update_last_used();
        assert!(account.last_used_at.is_some());
        assert!(account.last_used_at.unwrap() <= Utc::now());
    }

    #[test]
    fn should_manage_active_status() {
        let factory = StandardExchangeAccountFactory::default();
        let mut account = factory
            .create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            )
            .unwrap();

        assert!(account.is_active);

        account.deactivate();
        assert!(!account.is_active);

        account.activate();
        assert!(account.is_active);
    }

    #[test]
    fn should_check_permissions() {
        let factory = StandardExchangeAccountFactory::default();
        let account = factory
            .create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            )
            .unwrap();

        assert!(account.has_permission("spot_trading"));
        assert!(!account.has_permission("futures_trading"));
        assert!(!account.has_permission("withdrawals"));
        assert!(!account.has_permission("non_existent"));
    }

    #[test]
    fn should_update_permissions_with_validation() {
        let factory = StandardExchangeAccountFactory::default();
        let mut account = factory
            .create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            )
            .unwrap();

        let new_permissions = json!({
            "spot_trading": true,
            "futures_trading": true,
            "withdrawals": true
        });

        let validator = StandardExchangeValidator;
        let result = account.update_permissions(new_permissions.clone(), &validator);
        assert!(result.is_ok());
        assert_eq!(account.permissions, new_permissions);

        // Test invalid permissions
        let invalid_permissions = json!("invalid");
        let result = account.update_permissions(invalid_permissions, &validator);
        assert!(result.is_err());
    }

    #[test]
    fn canonical_exchange_name_normalizes() {
        assert_eq!(canonical_exchange_name("Bybit"), "bybit");
        assert_eq!(canonical_exchange_name("  HYPERLIQUID  "), "hyperliquid");
        assert_eq!(canonical_exchange_name("woo"), "woo");
        assert_eq!(canonical_exchange_name("Coinbase_Pro"), "coinbase_pro");
    }

    #[test]
    fn should_normalize_exchange_names() {
        let factory = StandardExchangeAccountFactory::default();

        let test_cases = vec![
            ("BINANCE", "binance"),
            ("Coinbase", "coinbase"),
            ("coinbase_pro", "coinbase_pro"),
            ("KRAKEN", "kraken"),
        ];

        for (input, expected) in test_cases {
            let account = factory
                .create_exchange_account(
                    mock_user_id(),
                    input,
                    mock_encrypted_key(),
                    mock_encrypted_secret(),
                    mock_permissions(),
                )
                .unwrap();

            assert_eq!(account.exchange_name, expected);
        }
    }

    #[test]
    fn should_provide_display_names() {
        let factory = StandardExchangeAccountFactory::default();

        let test_cases = vec![
            ("binance", "Binance"),
            ("coinbase", "Coinbase"),
            ("coinbase_pro", "Coinbase Pro"),
            ("kraken", "Kraken"),
        ];

        for (exchange_name, expected_display) in test_cases {
            let account = factory
                .create_exchange_account(
                    mock_user_id(),
                    exchange_name,
                    mock_encrypted_key(),
                    mock_encrypted_secret(),
                    mock_permissions(),
                )
                .unwrap();

            assert_eq!(account.exchange_display_name(), expected_display);
        }
    }

    #[test]
    fn should_support_dependency_injection() {
        // Custom validator for testing
        struct TestValidator;

        impl ExchangeValidator for TestValidator {
            fn validate_exchange_name(&self, name: &str) -> Result<(), ExchangeAccountError> {
                if name == "test_exchange" {
                    Ok(())
                } else {
                    Err(ExchangeAccountError::UnsupportedExchange(
                        "Only test_exchange allowed".to_string(),
                    ))
                }
            }

            fn validate_permissions(
                &self,
                permissions: &serde_json::Value,
            ) -> Result<(), ExchangeAccountError> {
                if permissions.is_object() {
                    Ok(())
                } else {
                    Err(ExchangeAccountError::InvalidPermissions(
                        "Must be object".to_string(),
                    ))
                }
            }
        }

        let factory = ExchangeAccountFactory::new(TestValidator);

        // Should accept test_exchange
        let result = factory.create_exchange_account(
            mock_user_id(),
            "test_exchange",
            mock_encrypted_key(),
            mock_encrypted_secret(),
            mock_permissions(),
        );
        assert!(result.is_ok());

        // Should reject binance (not in test validator)
        let result = factory.create_exchange_account(
            mock_user_id(),
            "binance",
            mock_encrypted_key(),
            mock_encrypted_secret(),
            mock_permissions(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn should_handle_deserialization_safely() {
        let factory = StandardExchangeAccountFactory::default();
        let account = factory
            .create_exchange_account(
                mock_user_id(),
                "binance",
                mock_encrypted_key(),
                mock_encrypted_secret(),
                mock_permissions(),
            )
            .unwrap();

        let serialized = serde_json::to_string(&account).expect("Should serialize");
        let deserialized: ExchangeAccount =
            serde_json::from_str(&serialized).expect("Should deserialize");

        // Non-sensitive fields should match
        assert_eq!(account.id, deserialized.id);
        assert_eq!(account.user_id, deserialized.user_id);
        assert_eq!(account.exchange_name, deserialized.exchange_name);
        assert_eq!(account.permissions, deserialized.permissions);

        // Sensitive fields should be empty after deserialization
        assert!(deserialized.encrypted_api_key.is_empty());
        assert!(deserialized.encrypted_secret.is_empty());
    }
}
