//! Repository error types for exchange account management
//!
//! This module defines comprehensive error handling for database operations,
//! ensuring that database failures are properly categorized and provide user-safe messages.

use common_utils::crypto::errors::EncryptionError;
use common_utils::ExchangeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("Database operation failed: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Encryption service error: {0}")]
    EncryptionError(#[from] EncryptionError),

    #[error("Exchange account not found")]
    NotFound,

    #[error("Duplicate exchange account - user already has account for this exchange")]
    DuplicateAccount,

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Transaction failed")]
    TransactionFailed,

    #[error("Invalid exchange account state")]
    InvalidState,
}

impl RepositoryError {
    /// Determines if the error indicates a critical system issue that should be logged
    pub fn is_critical(&self) -> bool {
        match self {
            RepositoryError::DatabaseError(_) => true,
            RepositoryError::TransactionFailed => true,
            RepositoryError::EncryptionError(e) => e.is_security_critical(),
            _ => false,
        }
    }

    /// Returns a user-safe error message that doesn't expose implementation details
    pub fn user_message(&self) -> &'static str {
        match self {
            RepositoryError::DatabaseError(_) => "Service temporarily unavailable",
            RepositoryError::EncryptionError(e) => e.user_message(),
            RepositoryError::NotFound => "Exchange account not found",
            RepositoryError::DuplicateAccount => "You already have an account for this exchange",
            RepositoryError::InvalidInput(_) => "Invalid input provided",
            RepositoryError::ConstraintViolation(_) => "Data validation failed",
            RepositoryError::TransactionFailed => "Service temporarily unavailable",
            RepositoryError::InvalidState => "Invalid account state",
        }
    }

    /// Creates a RepositoryError from a SQLx error, categorizing common constraint violations
    pub fn from_sqlx_error(error: sqlx::Error) -> Self {
        match &error {
            sqlx::Error::Database(db_error) => {
                let constraint = db_error.constraint();
                let code = db_error.code();

                // Handle PostgreSQL constraint violations
                if code.as_deref() == Some("23505") {
                    // Unique constraint violation
                    if constraint == Some("exchange_accounts_user_id_exchange_name_key") {
                        return RepositoryError::DuplicateAccount;
                    }
                }

                // Handle check constraint violations
                if code.as_deref() == Some("23514") {
                    // Check constraint violation
                    if let Some(constraint_name) = constraint {
                        let message = match constraint_name {
                            "check_exchange_name_supported" => "Unsupported exchange",
                            "check_exchange_name_not_empty" => "Exchange name cannot be empty",
                            "check_api_key_not_empty" => "API key cannot be empty",
                            "check_api_secret_not_empty" => "API secret cannot be empty",
                            "check_permissions_is_object" => "Invalid permissions format",
                            _ => "Data validation failed",
                        };
                        return RepositoryError::ConstraintViolation(message.to_string());
                    }
                }

                // Handle foreign key violations
                if code.as_deref() == Some("23503") {
                    // Foreign key constraint violation
                    return RepositoryError::InvalidInput("Invalid user ID".to_string());
                }

                RepositoryError::DatabaseError(error)
            }
            sqlx::Error::RowNotFound => RepositoryError::NotFound,
            _ => RepositoryError::DatabaseError(error),
        }
    }
}

// Conversion from RepositoryError to ExchangeError for unified error handling
impl From<RepositoryError> for ExchangeError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::DatabaseError(_) => {
                ExchangeError::ExchangeUnavailable("Database service unavailable".to_string())
            }
            RepositoryError::EncryptionError(encryption_error) => {
                ExchangeError::from(encryption_error)
            }
            RepositoryError::NotFound => {
                ExchangeError::AuthenticationError("Account not found".to_string())
            }
            RepositoryError::DuplicateAccount => {
                ExchangeError::OrderRejected("Duplicate account".to_string())
            }
            RepositoryError::InvalidInput(reason) => {
                ExchangeError::OrderRejected(format!("Invalid input: {}", reason))
            }
            RepositoryError::ConstraintViolation(reason) => {
                ExchangeError::OrderRejected(format!("Constraint violation: {}", reason))
            }
            RepositoryError::TransactionFailed => {
                ExchangeError::ExchangeUnavailable("Transaction failed".to_string())
            }
            RepositoryError::InvalidState => {
                ExchangeError::OrderRejected("Invalid state".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_utils::crypto::errors::EncryptionError;

    #[test]
    fn should_identify_critical_errors() {
        // Database errors should be critical
        let db_error = RepositoryError::DatabaseError(sqlx::Error::PoolClosed);
        assert!(db_error.is_critical());

        // Transaction failures should be critical
        assert!(RepositoryError::TransactionFailed.is_critical());

        // Security-critical encryption errors should be critical
        let critical_encryption = RepositoryError::EncryptionError(EncryptionError::TamperedData);
        assert!(critical_encryption.is_critical());

        // Non-critical errors
        assert!(!RepositoryError::NotFound.is_critical());
        assert!(!RepositoryError::DuplicateAccount.is_critical());
        assert!(!RepositoryError::InvalidInput("test".to_string()).is_critical());
    }

    #[test]
    fn should_provide_safe_user_messages() {
        let errors = vec![
            RepositoryError::DatabaseError(sqlx::Error::PoolClosed),
            RepositoryError::EncryptionError(EncryptionError::TamperedData),
            RepositoryError::NotFound,
            RepositoryError::DuplicateAccount,
            RepositoryError::InvalidInput("test".to_string()),
            RepositoryError::ConstraintViolation("test".to_string()),
            RepositoryError::TransactionFailed,
            RepositoryError::InvalidState,
        ];

        for error in errors {
            let message = error.user_message();
            assert!(!message.is_empty());
            // Ensure no technical details leak
            assert!(!message.contains("sqlx"));
            assert!(!message.contains("SQL"));
            assert!(!message.contains("postgres"));
            assert!(!message.contains("database"));
            assert!(!message.contains("pool"));
        }
    }

    #[test]
    fn should_handle_not_found_errors() {
        let not_found_error = sqlx::Error::RowNotFound;
        let repo_error = RepositoryError::from_sqlx_error(not_found_error);
        assert!(matches!(repo_error, RepositoryError::NotFound));
    }

    #[test]
    fn should_categorize_sqlx_errors() {
        // Test that we can create repository errors from SQLx errors
        let db_error = RepositoryError::DatabaseError(sqlx::Error::PoolClosed);
        assert!(db_error.is_critical());

        let not_found = RepositoryError::from_sqlx_error(sqlx::Error::RowNotFound);
        assert!(matches!(not_found, RepositoryError::NotFound));
    }

    #[test]
    fn test_repository_error_to_exchange_error_conversion() {
        // Test conversion of various RepositoryError types to ExchangeError
        let test_cases = vec![
            (
                RepositoryError::DatabaseError(sqlx::Error::PoolClosed),
                ExchangeError::ExchangeUnavailable("Database service unavailable".to_string()),
            ),
            (
                RepositoryError::NotFound,
                ExchangeError::AuthenticationError("Account not found".to_string()),
            ),
            (
                RepositoryError::DuplicateAccount,
                ExchangeError::OrderRejected("Duplicate account".to_string()),
            ),
            (
                RepositoryError::InvalidInput("bad data".to_string()),
                ExchangeError::OrderRejected("Invalid input: bad data".to_string()),
            ),
            (
                RepositoryError::ConstraintViolation("foreign key".to_string()),
                ExchangeError::OrderRejected("Constraint violation: foreign key".to_string()),
            ),
            (
                RepositoryError::TransactionFailed,
                ExchangeError::ExchangeUnavailable("Transaction failed".to_string()),
            ),
            (
                RepositoryError::InvalidState,
                ExchangeError::OrderRejected("Invalid state".to_string()),
            ),
        ];

        for (repo_error, expected_exchange_error) in test_cases {
            let converted: ExchangeError = repo_error.into();
            assert_eq!(converted, expected_exchange_error);
        }
    }

    #[test]
    fn test_encryption_error_propagation() {
        // Test that encryption errors are properly propagated through the conversion chain
        let encryption_error = EncryptionError::TamperedData;
        let repo_error = RepositoryError::EncryptionError(encryption_error);
        let exchange_error: ExchangeError = repo_error.into();

        // Should maintain the encryption error characteristics
        assert!(matches!(
            exchange_error,
            ExchangeError::ExchangeUnavailable(_)
        ));
        assert_eq!(exchange_error.status_code(), 503);
        assert!(exchange_error.is_retryable());
        assert_eq!(exchange_error.category(), "availability");
    }
}
