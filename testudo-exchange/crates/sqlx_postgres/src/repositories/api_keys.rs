//! Exchange Account Repository Implementation
//!
//! This module implements the repository pattern for managing exchange account data
//! with automatic encryption/decryption and comprehensive database operations.
//! Follows SOLID principles and provides comprehensive error handling.

// @anchor exchange:sqlx_postgres:api_keys
// @tags infra

use async_trait::async_trait;
use common_utils::crypto::vault::EncryptionService;
use common_utils::models::exchange_account::ExchangeAccount;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use super::errors::RepositoryError;
use super::types::*;

/// Repository trait for exchange account operations
/// Following Interface Segregation Principle - only what's needed for exchange accounts
#[async_trait]
pub trait ExchangeAccountRepository: Send + Sync {
    /// Creates a new exchange account with encrypted credentials
    async fn create(
        &self,
        request: CreateExchangeAccountRequest,
    ) -> Result<ExchangeAccount, RepositoryError>;

    /// Retrieves an exchange account by ID (without decrypted credentials)
    async fn get_by_id(&self, id: Uuid) -> Result<Option<ExchangeAccount>, RepositoryError>;

    /// Retrieves an exchange account by user and exchange name
    async fn get_by_user_and_exchange(
        &self,
        user_id: Uuid,
        exchange: &str,
    ) -> Result<Option<ExchangeAccount>, RepositoryError>;

    /// Retrieves exchange account with decrypted credentials (for API usage)
    async fn get_with_credentials(
        &self,
        id: Uuid,
    ) -> Result<Option<ExchangeAccountWithCredentials>, RepositoryError>;

    /// Lists exchange accounts for a user (without decrypted credentials)
    async fn list_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ExchangeAccountSummary>, RepositoryError>;

    /// Lists exchange accounts with filtering
    async fn list_with_filter(
        &self,
        filter: ExchangeAccountFilter,
    ) -> Result<Vec<ExchangeAccountSummary>, RepositoryError>;

    /// Updates an exchange account
    async fn update(
        &self,
        request: UpdateExchangeAccountRequest,
    ) -> Result<ExchangeAccount, RepositoryError>;

    /// Deletes an exchange account
    async fn delete(&self, id: Uuid) -> Result<bool, RepositoryError>;

    /// Updates the last_used_at timestamp
    async fn update_last_used(&self, id: Uuid) -> Result<(), RepositoryError>;

    /// Sets the active status of an exchange account
    async fn set_active_status(&self, id: Uuid, is_active: bool) -> Result<(), RepositoryError>;

    /// Counts exchange accounts for a user
    async fn count_by_user(&self, user_id: Uuid) -> Result<i64, RepositoryError>;

    /// Checks if a user has an account for a specific exchange
    async fn exists_for_user_and_exchange(
        &self,
        user_id: Uuid,
        exchange: &str,
    ) -> Result<bool, RepositoryError>;
}

/// PostgreSQL implementation of the ExchangeAccountRepository
pub struct PostgresExchangeAccountRepository {
    pool: PgPool,
    encryption_service: Arc<dyn EncryptionService>,
}

impl PostgresExchangeAccountRepository {
    /// Creates a new repository instance
    pub fn new(pool: PgPool, encryption_service: Arc<dyn EncryptionService>) -> Self {
        Self {
            pool,
            encryption_service,
        }
    }

    /// Encrypts credentials and creates the database representation
    async fn encrypt_credentials(
        &self,
        api_key: &str,
        api_secret: &str,
    ) -> Result<(Vec<u8>, Vec<u8>), RepositoryError> {
        let encrypted_key = self.encryption_service.encrypt(api_key).await?;
        let encrypted_secret = self.encryption_service.encrypt(api_secret).await?;
        Ok((encrypted_key, encrypted_secret))
    }

    /// Decrypts credentials from database representation
    async fn decrypt_credentials(
        &self,
        encrypted_key: &[u8],
        encrypted_secret: &[u8],
    ) -> Result<(String, String), RepositoryError> {
        let api_key = self.encryption_service.decrypt(encrypted_key).await?;
        let api_secret = self.encryption_service.decrypt(encrypted_secret).await?;
        Ok((api_key, api_secret))
    }

    /// Converts a database row to ExchangeAccount domain model
    fn row_to_exchange_account(&self, row: ExchangeAccountRow) -> ExchangeAccount {
        ExchangeAccount {
            id: row.id,
            user_id: row.user_id,
            exchange_name: row.exchange_name,
            encrypted_api_key: row.api_key_encrypted,
            encrypted_secret: row.api_secret_encrypted,
            permissions: row.permissions,
            is_active: row.is_active,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
            auth_mode: "api_key".to_string(),
            wallet_address: None,
        }
    }
}

#[async_trait]
impl ExchangeAccountRepository for PostgresExchangeAccountRepository {
    async fn create(
        &self,
        mut request: CreateExchangeAccountRequest,
    ) -> Result<ExchangeAccount, RepositoryError> {
        // Validate and normalize the request
        request
            .validate_and_normalize()
            .map_err(|e| RepositoryError::InvalidInput(format!("Validation failed: {}", e)))?;

        // Encrypt the credentials
        let (encrypted_key, encrypted_secret) = self
            .encrypt_credentials(&request.api_key, &request.api_secret)
            .await?;

        // Start a transaction for atomicity
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(RepositoryError::from_sqlx_error)?;

        let row = sqlx::query_as::<_, ExchangeAccountRow>(
            r#"
            INSERT INTO exchange_accounts (
                user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                permissions, is_active, created_at
            )
            VALUES ($1, $2, $3, $4, $5, true, NOW())
            RETURNING id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                      permissions, is_active, created_at, last_used_at
            "#,
        )
        .bind(request.user_id)
        .bind(&request.exchange_name)
        .bind(&encrypted_key)
        .bind(&encrypted_secret)
        .bind(&request.permissions)
        .fetch_one(&mut *tx)
        .await
        .map_err(RepositoryError::from_sqlx_error)?;

        // Commit the transaction
        tx.commit()
            .await
            .map_err(RepositoryError::from_sqlx_error)?;

        Ok(self.row_to_exchange_account(row))
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<ExchangeAccount>, RepositoryError> {
        let row = sqlx::query_as::<_, ExchangeAccountRow>(
            r#"
            SELECT id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                   permissions, is_active, created_at, last_used_at
            FROM exchange_accounts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from_sqlx_error)?;

        Ok(row.map(|r| self.row_to_exchange_account(r)))
    }

    async fn get_by_user_and_exchange(
        &self,
        user_id: Uuid,
        exchange: &str,
    ) -> Result<Option<ExchangeAccount>, RepositoryError> {
        let normalized_exchange = exchange.to_lowercase();

        let row = sqlx::query_as::<_, ExchangeAccountRow>(
            r#"
            SELECT id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                   permissions, is_active, created_at, last_used_at
            FROM exchange_accounts
            WHERE user_id = $1 AND exchange_name = $2
            "#,
        )
        .bind(user_id)
        .bind(&normalized_exchange)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from_sqlx_error)?;

        Ok(row.map(|r| self.row_to_exchange_account(r)))
    }

    async fn get_with_credentials(
        &self,
        id: Uuid,
    ) -> Result<Option<ExchangeAccountWithCredentials>, RepositoryError> {
        let row = sqlx::query_as::<_, ExchangeAccountRow>(
            r#"
            SELECT id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                   permissions, is_active, created_at, last_used_at
            FROM exchange_accounts
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(RepositoryError::from_sqlx_error)?;

        if let Some(row) = row {
            // Decrypt the credentials
            let (api_key, api_secret) = self
                .decrypt_credentials(&row.api_key_encrypted, &row.api_secret_encrypted)
                .await?;

            Ok(Some(ExchangeAccountWithCredentials {
                id: row.id,
                user_id: row.user_id,
                exchange_name: row.exchange_name,
                api_key,
                api_secret,
                permissions: row.permissions,
                is_active: row.is_active,
                created_at: row.created_at,
                last_used_at: row.last_used_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<ExchangeAccountSummary>, RepositoryError> {
        let filter = ExchangeAccountFilter::for_user(user_id);
        self.list_with_filter(filter).await
    }

    async fn list_with_filter(
        &self,
        filter: ExchangeAccountFilter,
    ) -> Result<Vec<ExchangeAccountSummary>, RepositoryError> {
        let base_query = r#"
            SELECT id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                   permissions, is_active, created_at, last_used_at
            FROM exchange_accounts
        "#;

        // Simplified filtering for now - can be enhanced with dynamic query building later
        let rows = if let Some(user_id) = filter.user_id {
            if let Some(exchange_name) = filter.exchange_name {
                // Filter by user and exchange
                sqlx::query_as::<_, ExchangeAccountRow>(&format!(
                    "{} WHERE user_id = $1 AND exchange_name = $2 ORDER BY created_at DESC",
                    base_query
                ))
                .bind(user_id)
                .bind(&exchange_name)
                .fetch_all(&self.pool)
                .await
                .map_err(RepositoryError::from_sqlx_error)?
            } else if let Some(is_active) = filter.is_active {
                // Filter by user and active status
                sqlx::query_as::<_, ExchangeAccountRow>(&format!(
                    "{} WHERE user_id = $1 AND is_active = $2 ORDER BY created_at DESC",
                    base_query
                ))
                .bind(user_id)
                .bind(is_active)
                .fetch_all(&self.pool)
                .await
                .map_err(RepositoryError::from_sqlx_error)?
            } else {
                // Filter by user only
                sqlx::query_as::<_, ExchangeAccountRow>(&format!(
                    "{} WHERE user_id = $1 ORDER BY created_at DESC",
                    base_query
                ))
                .bind(user_id)
                .fetch_all(&self.pool)
                .await
                .map_err(RepositoryError::from_sqlx_error)?
            }
        } else {
            // No filter - get all (should rarely be used)
            sqlx::query_as::<_, ExchangeAccountRow>(&format!(
                "{} ORDER BY created_at DESC",
                base_query
            ))
            .fetch_all(&self.pool)
            .await
            .map_err(RepositoryError::from_sqlx_error)?
        };

        let summaries = rows.into_iter().map(ExchangeAccountSummary::from).collect();

        Ok(summaries)
    }

    async fn update(
        &self,
        request: UpdateExchangeAccountRequest,
    ) -> Result<ExchangeAccount, RepositoryError> {
        // Validate the request
        request
            .validate_fields()
            .map_err(|e| RepositoryError::InvalidInput(format!("Validation failed: {}", e)))?;

        if !request.has_updates() {
            return Err(RepositoryError::InvalidInput(
                "No fields to update".to_string(),
            ));
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(RepositoryError::from_sqlx_error)?;

        // Handle credential updates (need encryption)
        if request.api_key.is_some() || request.api_secret.is_some() {
            // If updating either credential, we need both
            let current_row = sqlx::query_as::<_, ExchangeAccountRow>(
                "SELECT * FROM exchange_accounts WHERE id = $1",
            )
            .bind(request.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(RepositoryError::from_sqlx_error)?
            .ok_or(RepositoryError::NotFound)?;

            let (current_api_key, current_api_secret) = self
                .decrypt_credentials(
                    &current_row.api_key_encrypted,
                    &current_row.api_secret_encrypted,
                )
                .await?;

            let new_api_key = request.api_key.as_deref().unwrap_or(&current_api_key);
            let new_api_secret = request.api_secret.as_deref().unwrap_or(&current_api_secret);

            let (encrypted_key, encrypted_secret) = self
                .encrypt_credentials(new_api_key, new_api_secret)
                .await?;

            // Update with new encrypted credentials
            let row = if let Some(ref permissions) = request.permissions {
                if let Some(is_active) = request.is_active {
                    // Update all fields
                    sqlx::query_as::<_, ExchangeAccountRow>(
                        r#"
                        UPDATE exchange_accounts
                        SET api_key_encrypted = $2, api_secret_encrypted = $3, permissions = $4, is_active = $5
                        WHERE id = $1
                        RETURNING id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                                  permissions, is_active, created_at, last_used_at
                        "#
                    )
                    .bind(request.id)
                    .bind(&encrypted_key)
                    .bind(&encrypted_secret)
                    .bind(permissions)
                    .bind(is_active)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(RepositoryError::from_sqlx_error)?
                } else {
                    // Update credentials and permissions
                    sqlx::query_as::<_, ExchangeAccountRow>(
                        r#"
                        UPDATE exchange_accounts
                        SET api_key_encrypted = $2, api_secret_encrypted = $3, permissions = $4
                        WHERE id = $1
                        RETURNING id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                                  permissions, is_active, created_at, last_used_at
                        "#
                    )
                    .bind(request.id)
                    .bind(&encrypted_key)
                    .bind(&encrypted_secret)
                    .bind(permissions)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(RepositoryError::from_sqlx_error)?
                }
            } else if let Some(is_active) = request.is_active {
                // Update credentials and active status
                sqlx::query_as::<_, ExchangeAccountRow>(
                    r#"
                    UPDATE exchange_accounts
                    SET api_key_encrypted = $2, api_secret_encrypted = $3, is_active = $4
                    WHERE id = $1
                    RETURNING id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                              permissions, is_active, created_at, last_used_at
                    "#,
                )
                .bind(request.id)
                .bind(&encrypted_key)
                .bind(&encrypted_secret)
                .bind(is_active)
                .fetch_one(&mut *tx)
                .await
                .map_err(RepositoryError::from_sqlx_error)?
            } else {
                // Update credentials only
                sqlx::query_as::<_, ExchangeAccountRow>(
                    r#"
                    UPDATE exchange_accounts
                    SET api_key_encrypted = $2, api_secret_encrypted = $3
                    WHERE id = $1
                    RETURNING id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                              permissions, is_active, created_at, last_used_at
                    "#,
                )
                .bind(request.id)
                .bind(&encrypted_key)
                .bind(&encrypted_secret)
                .fetch_one(&mut *tx)
                .await
                .map_err(RepositoryError::from_sqlx_error)?
            };

            tx.commit()
                .await
                .map_err(RepositoryError::from_sqlx_error)?;
            Ok(self.row_to_exchange_account(row))
        } else {
            // Update non-credential fields only
            let row = if let Some(ref permissions) = request.permissions {
                if let Some(is_active) = request.is_active {
                    // Update permissions and active status
                    sqlx::query_as::<_, ExchangeAccountRow>(
                        r#"
                        UPDATE exchange_accounts
                        SET permissions = $2, is_active = $3
                        WHERE id = $1
                        RETURNING id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                                  permissions, is_active, created_at, last_used_at
                        "#
                    )
                    .bind(request.id)
                    .bind(permissions)
                    .bind(is_active)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(RepositoryError::from_sqlx_error)?
                } else {
                    // Update permissions only
                    sqlx::query_as::<_, ExchangeAccountRow>(
                        r#"
                        UPDATE exchange_accounts
                        SET permissions = $2
                        WHERE id = $1
                        RETURNING id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                                  permissions, is_active, created_at, last_used_at
                        "#
                    )
                    .bind(request.id)
                    .bind(permissions)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(RepositoryError::from_sqlx_error)?
                }
            } else if let Some(is_active) = request.is_active {
                // Update active status only
                sqlx::query_as::<_, ExchangeAccountRow>(
                    r#"
                    UPDATE exchange_accounts
                    SET is_active = $2
                    WHERE id = $1
                    RETURNING id, user_id, exchange_name, api_key_encrypted, api_secret_encrypted,
                              permissions, is_active, created_at, last_used_at
                    "#,
                )
                .bind(request.id)
                .bind(is_active)
                .fetch_one(&mut *tx)
                .await
                .map_err(RepositoryError::from_sqlx_error)?
            } else {
                return Err(RepositoryError::InvalidInput(
                    "No valid fields to update".to_string(),
                ));
            };

            tx.commit()
                .await
                .map_err(RepositoryError::from_sqlx_error)?;
            Ok(self.row_to_exchange_account(row))
        }
    }

    async fn delete(&self, id: Uuid) -> Result<bool, RepositoryError> {
        let result = sqlx::query("DELETE FROM exchange_accounts WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from_sqlx_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn update_last_used(&self, id: Uuid) -> Result<(), RepositoryError> {
        let result = sqlx::query("UPDATE exchange_accounts SET last_used_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn set_active_status(&self, id: Uuid, is_active: bool) -> Result<(), RepositoryError> {
        let result = sqlx::query("UPDATE exchange_accounts SET is_active = $2 WHERE id = $1")
            .bind(id)
            .bind(is_active)
            .execute(&self.pool)
            .await
            .map_err(RepositoryError::from_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    async fn count_by_user(&self, user_id: Uuid) -> Result<i64, RepositoryError> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM exchange_accounts WHERE user_id = $1")
                .bind(user_id)
                .fetch_one(&self.pool)
                .await
                .map_err(RepositoryError::from_sqlx_error)?;

        Ok(count.0)
    }

    async fn exists_for_user_and_exchange(
        &self,
        user_id: Uuid,
        exchange: &str,
    ) -> Result<bool, RepositoryError> {
        let normalized_exchange = exchange.to_lowercase();

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM exchange_accounts WHERE user_id = $1 AND exchange_name = $2",
        )
        .bind(user_id)
        .bind(&normalized_exchange)
        .fetch_one(&self.pool)
        .await
        .map_err(RepositoryError::from_sqlx_error)?;

        Ok(count.0 > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common_utils::crypto::errors::EncryptionError;
    use serde_json::json;
    use std::env;

    // Mock encryption service for testing
    struct MockEncryptionService;

    #[async_trait::async_trait]
    impl EncryptionService for MockEncryptionService {
        async fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, EncryptionError> {
            // Simple mock: just reverse the string and convert to bytes
            Ok(plaintext.chars().rev().collect::<String>().into_bytes())
        }

        async fn decrypt(&self, ciphertext: &[u8]) -> Result<String, EncryptionError> {
            // Simple mock: convert to string and reverse
            let reversed = String::from_utf8(ciphertext.to_vec())
                .map_err(|_| EncryptionError::DecryptionFailed)?;
            Ok(reversed.chars().rev().collect())
        }

        fn generate_key(&self) -> Result<[u8; 32], EncryptionError> {
            Ok([0u8; 32])
        }
    }

    // Test helper functions
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

    fn create_test_request() -> CreateExchangeAccountRequest {
        CreateExchangeAccountRequest {
            user_id: mock_user_id(),
            exchange_name: "binance".to_string(),
            api_key: "test_api_key_123".to_string(),
            api_secret: "test_api_secret_456".to_string(),
            permissions: mock_permissions(),
        }
    }

    #[test]
    fn should_handle_encryption_errors() {
        // This test verifies that encryption errors are properly propagated
        // through the RepositoryError type system

        // Test a security-critical encryption error
        let critical_encryption_error = EncryptionError::TamperedData;
        let critical_repo_error = RepositoryError::EncryptionError(critical_encryption_error);
        assert!(critical_repo_error.is_critical());

        // Test a non-critical encryption error
        let non_critical_encryption_error = EncryptionError::EncryptionFailed;
        let non_critical_repo_error =
            RepositoryError::EncryptionError(non_critical_encryption_error);
        assert!(!non_critical_repo_error.is_critical());

        // Both should provide user-safe messages
        assert_eq!(
            critical_repo_error.user_message(),
            "Encryption service temporarily unavailable"
        );
        assert_eq!(
            non_critical_repo_error.user_message(),
            "Encryption service temporarily unavailable"
        );
    }

    #[test]
    fn should_validate_request_types() {
        // Test that our request types work properly
        let mut request = create_test_request();
        assert!(request.validate_and_normalize().is_ok());
        assert_eq!(request.exchange_name, "binance");

        // Test update request
        let update_request = UpdateExchangeAccountRequest {
            id: mock_user_id(),
            api_key: Some("new_key".to_string()),
            api_secret: None,
            permissions: None,
            is_active: Some(false),
        };
        assert!(update_request.validate_fields().is_ok());
        assert!(update_request.has_updates());
    }

    #[tokio::test]
    async fn should_create_repository_with_mock_components() {
        // Test that we can create a repository instance with mocked dependencies
        // This test doesn't require a database connection
        let encryption_service: Arc<dyn EncryptionService> = Arc::new(MockEncryptionService);

        // Create a mock pool (this will be a placeholder that can't actually connect)
        // We use a connection string that won't actually work, but allows construction
        let database_url = "postgres://mock:mock@localhost:0/mock";

        // The pool creation will work but connection attempts will fail
        // This is fine for testing construction
        let pool_options = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_millis(1));

        // Just test that we can create the repository structure
        // We don't actually connect to verify it works
        let _repo = PostgresExchangeAccountRepository::new(
            pool_options.connect_lazy(database_url).unwrap(),
            encryption_service,
        );

        // If we get here, construction succeeded (no explicit assert needed)
    }

    // Note: Integration tests that require a real database connection
    // would be placed here but marked with #[ignore] for CI

    #[tokio::test]
    #[ignore] // Skip in CI - requires database
    async fn should_create_exchange_account_with_encryption() {
        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://root:root@localhost:5000/exchange-db".to_string());

        let pool = match sqlx::PgPool::connect(&database_url).await {
            Ok(pool) => pool,
            Err(_) => {
                eprintln!("Database not available, skipping integration test");
                return;
            }
        };

        let encryption_service: Arc<dyn EncryptionService> = Arc::new(MockEncryptionService);
        let repo = PostgresExchangeAccountRepository::new(pool, encryption_service);
        let request = create_test_request();

        let account = repo
            .create(request.clone())
            .await
            .expect("Should create account");

        assert_eq!(account.user_id, request.user_id);
        assert_eq!(account.exchange_name, "binance");
        assert!(!account.encrypted_api_key.is_empty());
        assert!(!account.encrypted_secret.is_empty());
        assert!(account.is_active);
        assert!(account.last_used_at.is_none());

        // Clean up
        let _ = repo.delete(account.id).await;
    }
}
