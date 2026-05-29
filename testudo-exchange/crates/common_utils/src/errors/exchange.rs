//! Exchange error types for comprehensive error handling
//!
//! This module provides centralized error handling for all exchange operations,
//! following SOLID principles and providing user-safe error messages.
//!
//! # Usage
//!
//! ```rust
//! use common_utils::errors::ExchangeError;
//! use rust_decimal::Decimal;
//!
//! let error = ExchangeError::InsufficientBalance {
//!     required: Decimal::new(100, 0),
//!     available: Decimal::new(50, 0),
//! };
//!
//! assert_eq!(error.status_code(), 400);
//! assert_eq!(error.category(), "balance");
//! assert!(!error.is_retryable());
//! ```
//!
//! # Error Conversion
//!
//! Other error types can be converted to `ExchangeError` by implementing `From` traits
//! in their respective modules:
//!
//! ```rust,ignore
//! impl From<RoutingError> for ExchangeError {
//!     fn from(error: RoutingError) -> Self {
//!         match error {
//!             RoutingError::Timeout => ExchangeError::ConnectionError("Timeout".to_string()),
//!             // ... other mappings
//!         }
//!     }
//! }
//! ```

// @anchor exchange:common_utils:exchange
// @tags infra

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExchangeError {
    #[error("Connection failed: {0}")]
    ConnectionError(String),

    #[error("Authentication failed: {0}")]
    AuthenticationError(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance {
        required: Decimal,
        available: Decimal,
    },

    #[error("Order rejected: {0}")]
    OrderRejected(String),

    #[error("Rate limited, retry after {0:?}")]
    RateLimited(Duration),

    #[error("Exchange unavailable: {0}")]
    ExchangeUnavailable(String),
}

impl ExchangeError {
    /// Maps error to appropriate HTTP status code
    pub fn status_code(&self) -> u16 {
        match self {
            ExchangeError::ConnectionError(_) => 503,
            ExchangeError::AuthenticationError(_) => 401,
            ExchangeError::InsufficientBalance { .. } => 400,
            ExchangeError::OrderRejected(_) => 422,
            ExchangeError::RateLimited(_) => 429,
            ExchangeError::ExchangeUnavailable(_) => 503,
        }
    }

    /// Returns user-safe error message (no technical details)
    pub fn user_message(&self) -> String {
        match self {
            ExchangeError::ConnectionError(_) => "Service temporarily unavailable".to_string(),
            ExchangeError::AuthenticationError(_) => "Authentication failed".to_string(),
            ExchangeError::InsufficientBalance {
                required,
                available,
            } => {
                format!(
                    "Insufficient balance: required {}, available {}",
                    required, available
                )
            }
            ExchangeError::OrderRejected(reason) => format!("Order rejected: {}", reason),
            ExchangeError::RateLimited(duration) => {
                format!("Rate limited, retry after {} seconds", duration.as_secs())
            }
            ExchangeError::ExchangeUnavailable(_) => "Exchange temporarily unavailable".to_string(),
        }
    }

    /// Determines if error indicates a temporary condition
    pub fn is_retryable(&self) -> bool {
        match self {
            ExchangeError::ConnectionError(_) => true,
            ExchangeError::AuthenticationError(_) => false,
            ExchangeError::InsufficientBalance { .. } => false,
            ExchangeError::OrderRejected(_) => false,
            ExchangeError::RateLimited(_) => true,
            ExchangeError::ExchangeUnavailable(_) => true,
        }
    }

    /// Returns error category for metrics/logging
    pub fn category(&self) -> &'static str {
        match self {
            ExchangeError::ConnectionError(_) => "connection",
            ExchangeError::AuthenticationError(_) => "authentication",
            ExchangeError::InsufficientBalance { .. } => "balance",
            ExchangeError::OrderRejected(_) => "order",
            ExchangeError::RateLimited(_) => "rate_limit",
            ExchangeError::ExchangeUnavailable(_) => "availability",
        }
    }
}

// Error conversion traits for existing common_utils errors
impl From<crate::crypto::errors::EncryptionError> for ExchangeError {
    fn from(error: crate::crypto::errors::EncryptionError) -> Self {
        ExchangeError::ExchangeUnavailable(format!("Encryption service error: {}", error))
    }
}

// Conversion from order validation errors
impl From<crate::types::OrderValidationError> for ExchangeError {
    fn from(error: crate::types::OrderValidationError) -> Self {
        ExchangeError::OrderRejected(format!("Order validation failed: {}", error))
    }
}

// Conversion from user errors
impl From<crate::models::UserError> for ExchangeError {
    fn from(error: crate::models::UserError) -> Self {
        ExchangeError::AuthenticationError(format!("User error: {}", error))
    }
}

impl From<crate::models::ExchangeAccountError> for ExchangeError {
    fn from(error: crate::models::ExchangeAccountError) -> Self {
        ExchangeError::AuthenticationError(format!("Exchange account error: {}", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use std::time::Duration;

    #[test]
    fn test_error_display_formatting() {
        let error = ExchangeError::InsufficientBalance {
            required: Decimal::new(100, 0),
            available: Decimal::new(50, 0),
        };
        let display = format!("{}", error);
        assert_eq!(display, "Insufficient balance: required 100, available 50");

        let connection_error = ExchangeError::ConnectionError("timeout".to_string());
        assert_eq!(
            format!("{}", connection_error),
            "Connection failed: timeout"
        );
    }

    #[test]
    fn test_status_code_mapping() {
        assert_eq!(
            ExchangeError::ConnectionError("test".to_string()).status_code(),
            503
        );
        assert_eq!(
            ExchangeError::AuthenticationError("test".to_string()).status_code(),
            401
        );
        assert_eq!(
            ExchangeError::InsufficientBalance {
                required: Decimal::new(100, 0),
                available: Decimal::new(50, 0)
            }
            .status_code(),
            400
        );
        assert_eq!(
            ExchangeError::OrderRejected("test".to_string()).status_code(),
            422
        );
        assert_eq!(
            ExchangeError::RateLimited(Duration::from_secs(60)).status_code(),
            429
        );
        assert_eq!(
            ExchangeError::ExchangeUnavailable("test".to_string()).status_code(),
            503
        );
    }

    #[test]
    fn test_user_message_safety() {
        let error =
            ExchangeError::ConnectionError("Internal DB connection pool exhausted".to_string());
        let user_msg = error.user_message();

        // Should not expose technical details
        assert_eq!(user_msg, "Service temporarily unavailable");
        assert!(!user_msg.contains("DB"));
        assert!(!user_msg.contains("pool"));
        assert!(!user_msg.contains("Internal"));

        let balance_error = ExchangeError::InsufficientBalance {
            required: Decimal::new(100, 2), // 1.00
            available: Decimal::new(50, 2), // 0.50
        };
        assert_eq!(
            balance_error.user_message(),
            "Insufficient balance: required 1.00, available 0.50"
        );
    }

    #[test]
    fn test_retryability_logic() {
        assert!(ExchangeError::ConnectionError("test".to_string()).is_retryable());
        assert!(!ExchangeError::AuthenticationError("test".to_string()).is_retryable());
        assert!(!ExchangeError::InsufficientBalance {
            required: Decimal::new(100, 0),
            available: Decimal::new(50, 0)
        }
        .is_retryable());
        assert!(!ExchangeError::OrderRejected("test".to_string()).is_retryable());
        assert!(ExchangeError::RateLimited(Duration::from_secs(60)).is_retryable());
        assert!(ExchangeError::ExchangeUnavailable("test".to_string()).is_retryable());
    }

    #[test]
    fn test_error_categorization() {
        assert_eq!(
            ExchangeError::ConnectionError("test".to_string()).category(),
            "connection"
        );
        assert_eq!(
            ExchangeError::AuthenticationError("test".to_string()).category(),
            "authentication"
        );
        assert_eq!(
            ExchangeError::InsufficientBalance {
                required: Decimal::new(100, 0),
                available: Decimal::new(50, 0)
            }
            .category(),
            "balance"
        );
        assert_eq!(
            ExchangeError::OrderRejected("test".to_string()).category(),
            "order"
        );
        assert_eq!(
            ExchangeError::RateLimited(Duration::from_secs(60)).category(),
            "rate_limit"
        );
        assert_eq!(
            ExchangeError::ExchangeUnavailable("test".to_string()).category(),
            "availability"
        );
    }

    #[test]
    fn test_error_conversion_from_encryption_error() {
        use crate::crypto::errors::EncryptionError;

        let encryption_error = EncryptionError::TamperedData;
        let exchange_error: ExchangeError = encryption_error.into();

        assert!(matches!(
            exchange_error,
            ExchangeError::ExchangeUnavailable(_)
        ));
        assert_eq!(exchange_error.status_code(), 503);
        assert!(exchange_error.is_retryable());
    }

    #[test]
    fn test_error_conversion_from_order_validation_error() {
        use crate::types::OrderValidationError;

        let validation_error = OrderValidationError::InvalidSymbol("INVALID".to_string());
        let exchange_error: ExchangeError = validation_error.into();

        assert!(matches!(exchange_error, ExchangeError::OrderRejected(_)));
        assert_eq!(exchange_error.status_code(), 422);
        assert!(!exchange_error.is_retryable());
    }

    #[test]
    fn test_error_conversion_from_user_error() {
        use crate::models::UserError;

        let user_error = UserError::ValidationFailed("Invalid input".to_string());
        let exchange_error: ExchangeError = user_error.into();

        assert!(matches!(
            exchange_error,
            ExchangeError::AuthenticationError(_)
        ));
        assert_eq!(exchange_error.status_code(), 401);
        assert!(!exchange_error.is_retryable());
    }

    #[test]
    fn test_serialization_support() {
        use serde_json;

        let error = ExchangeError::InsufficientBalance {
            required: Decimal::new(100, 0),
            available: Decimal::new(50, 0),
        };

        // Test serialization
        let serialized = serde_json::to_string(&error).expect("Should serialize");
        assert!(!serialized.is_empty());

        // Test deserialization
        let deserialized: ExchangeError =
            serde_json::from_str(&serialized).expect("Should deserialize");
        assert_eq!(error, deserialized);
    }

    #[test]
    fn test_rate_limit_duration_formatting() {
        let error = ExchangeError::RateLimited(Duration::from_secs(120));
        let user_msg = error.user_message();
        assert_eq!(user_msg, "Rate limited, retry after 120 seconds");

        let short_limit = ExchangeError::RateLimited(Duration::from_secs(1));
        assert_eq!(
            short_limit.user_message(),
            "Rate limited, retry after 1 seconds"
        );
    }

    #[test]
    fn test_clone_and_equality() {
        let error1 = ExchangeError::OrderRejected("Invalid price".to_string());
        let error2 = error1.clone();

        assert_eq!(error1, error2);
        assert_eq!(error1.status_code(), error2.status_code());
        assert_eq!(error1.category(), error2.category());
    }

    #[test]
    fn test_comprehensive_error_coverage() {
        // Test that we have comprehensive coverage of all common error scenarios
        let test_cases = vec![
            (
                ExchangeError::ConnectionError("Network failure".to_string()),
                503,
                "connection",
                true,
            ),
            (
                ExchangeError::AuthenticationError("Invalid credentials".to_string()),
                401,
                "authentication",
                false,
            ),
            (
                ExchangeError::InsufficientBalance {
                    required: Decimal::new(1000, 2),
                    available: Decimal::new(500, 2),
                },
                400,
                "balance",
                false,
            ),
            (
                ExchangeError::OrderRejected("Invalid price".to_string()),
                422,
                "order",
                false,
            ),
            (
                ExchangeError::RateLimited(Duration::from_secs(300)),
                429,
                "rate_limit",
                true,
            ),
            (
                ExchangeError::ExchangeUnavailable("Maintenance".to_string()),
                503,
                "availability",
                true,
            ),
        ];

        for (error, expected_status, expected_category, expected_retryable) in test_cases {
            assert_eq!(error.status_code(), expected_status);
            assert_eq!(error.category(), expected_category);
            assert_eq!(error.is_retryable(), expected_retryable);
            assert!(!error.user_message().is_empty());
        }
    }

    #[test]
    fn test_error_boundary_conditions() {
        // Test edge cases and boundary conditions

        // Zero balance
        let zero_balance = ExchangeError::InsufficientBalance {
            required: Decimal::new(1, 0),
            available: Decimal::new(0, 0),
        };
        assert_eq!(
            zero_balance.user_message(),
            "Insufficient balance: required 1, available 0"
        );

        // Very short rate limit
        let short_rate_limit = ExchangeError::RateLimited(Duration::from_millis(500));
        assert_eq!(
            short_rate_limit.user_message(),
            "Rate limited, retry after 0 seconds"
        );

        // Empty strings
        let empty_connection = ExchangeError::ConnectionError("".to_string());
        assert_eq!(
            empty_connection.user_message(),
            "Service temporarily unavailable"
        );

        let empty_order_rejection = ExchangeError::OrderRejected("".to_string());
        assert_eq!(empty_order_rejection.user_message(), "Order rejected: ");
    }

    #[test]
    fn test_error_semantic_compression() {
        // Following Numogrammatic Codex - test the semantic compression and abstraction discovery

        // Multiple technical errors should map to same user-safe semantic category
        let technical_errors = vec![
            ExchangeError::ConnectionError("TCP timeout".to_string()),
            ExchangeError::ConnectionError("DNS resolution failed".to_string()),
            ExchangeError::ConnectionError("SSL handshake error".to_string()),
        ];

        for error in technical_errors {
            assert_eq!(error.user_message(), "Service temporarily unavailable");
            assert_eq!(error.category(), "connection");
            assert!(error.is_retryable());
        }

        // Different balance scenarios should maintain semantic consistency
        let balance_errors = vec![
            ExchangeError::InsufficientBalance {
                required: Decimal::new(100, 0),
                available: Decimal::new(0, 0),
            },
            ExchangeError::InsufficientBalance {
                required: Decimal::new(1, 3),
                available: Decimal::new(999, 6),
            },
        ];

        for error in balance_errors {
            assert_eq!(error.category(), "balance");
            assert!(!error.is_retryable());
            assert_eq!(error.status_code(), 400);
        }
    }

    #[test]
    fn test_solid_principles_adherence() {
        // Single Responsibility: Each error variant has one clear purpose
        let connection_error = ExchangeError::ConnectionError("test".to_string());
        assert_eq!(connection_error.category(), "connection");
        assert_ne!(connection_error.category(), "authentication"); // Doesn't mix concerns

        // Open/Closed: Can extend with new variants without modifying existing code
        // This is demonstrated by the enum design allowing new variants

        // Liskov Substitution: All variants behave consistently with the interface
        let errors: Vec<ExchangeError> = vec![
            ExchangeError::ConnectionError("test".to_string()),
            ExchangeError::AuthenticationError("test".to_string()),
            ExchangeError::OrderRejected("test".to_string()),
        ];

        for error in errors {
            // All variants support the same interface consistently
            assert!(!error.user_message().is_empty());
            assert!(error.status_code() >= 400 && error.status_code() < 600);
            assert!(!error.category().is_empty());
            // is_retryable() can be true or false, but always returns a bool
        }

        // Interface Segregation: Clean, focused methods
        let error = ExchangeError::RateLimited(Duration::from_secs(60));

        // Methods are focused and don't expose unnecessary details
        assert_eq!(error.status_code(), 429); // Just HTTP status
        assert_eq!(error.category(), "rate_limit"); // Just category
        assert!(error.is_retryable()); // Just retryability
        assert_eq!(error.user_message(), "Rate limited, retry after 60 seconds");
        // Just user message

        // Dependency Inversion: Error handling depends on abstractions (traits)
        // This is demonstrated by the From trait implementations
    }
}
