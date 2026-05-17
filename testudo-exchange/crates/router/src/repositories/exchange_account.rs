use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    AeadCore, Aes256Gcm,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::types::exchange_names::{auth_modes, exchanges};

/// AES-256-GCM vault for encrypting/decrypting exchange credentials.
///
/// Format: [12-byte nonce | ciphertext + auth tag]
#[derive(Clone)]
pub struct AesGcmVault {
    cipher: Aes256Gcm,
}

impl AesGcmVault {
    /// Create from `CREDENTIAL_ENCRYPTION_KEY` env var (64 hex chars = 32 bytes).
    pub fn from_env() -> Result<Self, VaultError> {
        let key_hex =
            std::env::var("CREDENTIAL_ENCRYPTION_KEY").map_err(|_| VaultError::MissingKey)?;
        Self::from_hex(&key_hex)
    }

    /// Create from a hex-encoded 32-byte key.
    pub fn from_hex(hex_key: &str) -> Result<Self, VaultError> {
        let key_bytes = hex::decode(hex_key)
            .map_err(|_| VaultError::InvalidKey("invalid hex encoding".to_string()))?;
        if key_bytes.len() != 32 {
            return Err(VaultError::InvalidKey(format!(
                "key must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }
        let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&key_bytes);
        Ok(Self {
            cipher: Aes256Gcm::new(key),
        })
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| VaultError::EncryptionFailed)?;
        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, VaultError> {
        if data.len() < 12 {
            return Err(VaultError::DecryptionFailed);
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = aes_gcm::Nonce::from_slice(nonce_bytes);
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| VaultError::DecryptionFailed)
    }
}

impl std::fmt::Debug for AesGcmVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AesGcmVault").finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("CREDENTIAL_ENCRYPTION_KEY env var not set")]
    MissingKey,
    #[error("Invalid encryption key: {0}")]
    InvalidKey(String),
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed")]
    DecryptionFailed,
}

/// Row type for exchange_accounts table (no encrypted fields in listing queries).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExchangeAccountRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub exchange_name: String,
    pub permissions: Option<serde_json::Value>,
    pub is_active: Option<bool>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    pub auth_mode: String,
    pub wallet_address: Option<String>,
}

/// Row type for loading encrypted credentials.
#[derive(Debug, sqlx::FromRow)]
struct CredentialRow {
    api_key_encrypted: Vec<u8>,
    api_secret_encrypted: Vec<u8>,
    passphrase_encrypted: Option<Vec<u8>>,
    exchange_name: String,
    auth_mode: String,
    wallet_address: Option<String>,
}

/// Decrypted credentials for making API calls.
pub struct DecryptedCredentials {
    pub exchange_name: String,
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: Option<String>,
    pub auth_mode: String,
    pub wallet_address: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Account not found")]
    NotFound,
    #[error("Account already exists for exchange: {0}")]
    DuplicateAccount(String),
    #[error("State conflict: {0}")]
    Conflict(String),
}

/// Repository for exchange account CRUD with encrypted credentials.
#[derive(Clone)]
pub struct ExchangeAccountRepository {
    pool: PgPool,
    vault: AesGcmVault,
}

impl ExchangeAccountRepository {
    pub fn new(pool: PgPool, vault: AesGcmVault) -> Self {
        Self { pool, vault }
    }

    /// List exchange accounts for a user.
    /// Returns all active accounts PLUS inactive agent wallets (which need re-authorization).
    /// Non-agent-wallet inactive accounts are excluded.
    pub async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<ExchangeAccountRow>, RepoError> {
        sqlx::query_as::<_, ExchangeAccountRow>(
            "SELECT id, user_id, exchange_name, permissions, is_active, created_at, last_used_at, \
             auth_mode, wallet_address \
             FROM exchange_accounts WHERE user_id = $1 \
             AND (is_active = true OR (auth_mode = 'agent_wallet' AND is_active = false)) \
             ORDER BY is_active DESC, created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))
    }

    /// Insert a new exchange account with encrypted credentials.
    pub async fn insert(
        &self,
        user_id: Uuid,
        exchange_name: &str,
        api_key: &str,
        api_secret: &str,
        passphrase: Option<&str>,
        permissions: serde_json::Value,
    ) -> Result<ExchangeAccountRow, RepoError> {
        let encrypted_key = self
            .vault
            .encrypt(api_key.as_bytes())
            .map_err(|e| RepoError::Encryption(e.to_string()))?;
        let encrypted_secret = self
            .vault
            .encrypt(api_secret.as_bytes())
            .map_err(|e| RepoError::Encryption(e.to_string()))?;
        let encrypted_passphrase = passphrase
            .map(|p| self.vault.encrypt(p.as_bytes()))
            .transpose()
            .map_err(|e| RepoError::Encryption(e.to_string()))?;

        sqlx::query_as::<_, ExchangeAccountRow>(
            "INSERT INTO exchange_accounts (user_id, exchange_name, api_key_encrypted, api_secret_encrypted, passphrase_encrypted, permissions) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             RETURNING id, user_id, exchange_name, permissions, is_active, created_at, last_used_at, auth_mode, wallet_address"
        )
        .bind(user_id)
        .bind(exchange_name)
        .bind(&encrypted_key)
        .bind(&encrypted_secret)
        .bind(&encrypted_passphrase)
        .bind(&permissions)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))
    }

    /// Delete an exchange account with ownership verification.
    pub async fn delete(&self, account_id: Uuid, user_id: Uuid) -> Result<bool, RepoError> {
        let result = sqlx::query("DELETE FROM exchange_accounts WHERE id = $1 AND user_id = $2")
            .bind(account_id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    /// Decrypt a credential row into plaintext credentials.
    fn decrypt_credential_row(&self, row: CredentialRow) -> Result<DecryptedCredentials, RepoError> {
        let api_key = self
            .vault
            .decrypt(&row.api_key_encrypted)
            .map_err(|e| RepoError::Encryption(e.to_string()))?;
        let api_secret = self
            .vault
            .decrypt(&row.api_secret_encrypted)
            .map_err(|e| RepoError::Encryption(e.to_string()))?;
        let passphrase = row
            .passphrase_encrypted
            .map(|enc| {
                let bytes = self
                    .vault
                    .decrypt(&enc)
                    .map_err(|e| RepoError::Encryption(e.to_string()))?;
                String::from_utf8(bytes)
                    .map_err(|_| RepoError::Encryption("invalid utf8 in passphrase".to_string()))
            })
            .transpose()?;

        Ok(DecryptedCredentials {
            exchange_name: row.exchange_name,
            api_key: String::from_utf8(api_key)
                .map_err(|_| RepoError::Encryption("invalid utf8 in api_key".to_string()))?,
            api_secret: String::from_utf8(api_secret)
                .map_err(|_| RepoError::Encryption("invalid utf8 in api_secret".to_string()))?,
            passphrase,
            auth_mode: row.auth_mode,
            wallet_address: row.wallet_address,
        })
    }

    /// Load and decrypt credentials for an account.
    pub async fn load_credentials(
        &self,
        account_id: Uuid,
        user_id: Uuid,
    ) -> Result<DecryptedCredentials, RepoError> {
        let row: CredentialRow = sqlx::query_as::<_, CredentialRow>(
            "SELECT api_key_encrypted, api_secret_encrypted, passphrase_encrypted, exchange_name, \
             auth_mode, wallet_address \
             FROM exchange_accounts WHERE id = $1 AND user_id = $2 AND is_active = true",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?
        .ok_or(RepoError::NotFound)?;

        self.decrypt_credential_row(row)
    }

    /// Find first active account for a user on a given exchange.
    pub async fn find_by_exchange(
        &self,
        user_id: Uuid,
        exchange_name: &str,
    ) -> Result<Option<ExchangeAccountRow>, RepoError> {
        sqlx::query_as::<_, ExchangeAccountRow>(
            "SELECT id, user_id, exchange_name, permissions, is_active, created_at, last_used_at, \
             auth_mode, wallet_address \
             FROM exchange_accounts WHERE user_id = $1 AND exchange_name = $2 AND is_active = true \
             LIMIT 1",
        )
        .bind(user_id)
        .bind(exchange_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))
    }

    /// Insert a new agent wallet account with encrypted agent key.
    /// The agent_key is the generated private key; agent_address is its derived ETH address.
    /// wallet_address is the user's main wallet address (for info queries + WS subscriptions).
    /// Account starts with is_active = false; activated after EIP-712 approval (AW-02).
    pub async fn insert_agent_wallet(
        &self,
        user_id: Uuid,
        wallet_address: &str,
        agent_key: &str,
        agent_address: &str,
    ) -> Result<ExchangeAccountRow, RepoError> {
        let encrypted_key = self
            .vault
            .encrypt(agent_address.as_bytes())
            .map_err(|e| RepoError::Encryption(e.to_string()))?;
        let encrypted_secret = self
            .vault
            .encrypt(agent_key.as_bytes())
            .map_err(|e| RepoError::Encryption(e.to_string()))?;

        sqlx::query_as::<_, ExchangeAccountRow>(
            "INSERT INTO exchange_accounts \
             (user_id, exchange_name, api_key_encrypted, api_secret_encrypted, auth_mode, wallet_address, permissions, is_active) \
             VALUES ($1, $2, $3, $4, $5, $6, '{}'::jsonb, false) \
             RETURNING id, user_id, exchange_name, permissions, is_active, created_at, last_used_at, auth_mode, wallet_address"
        )
        .bind(user_id)
        .bind(exchanges::HYPERLIQUID)
        .bind(&encrypted_key)
        .bind(&encrypted_secret)
        .bind(auth_modes::AGENT_WALLET)
        .bind(wallet_address)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))
    }

    /// Load credentials for a pending agent wallet account (is_active = false).
    /// Used by the AW-02 approval flow before the account is activated.
    pub async fn load_credentials_for_approval(
        &self,
        account_id: Uuid,
        user_id: Uuid,
    ) -> Result<DecryptedCredentials, RepoError> {
        let row: CredentialRow = sqlx::query_as::<_, CredentialRow>(
            "SELECT api_key_encrypted, api_secret_encrypted, passphrase_encrypted, exchange_name, \
             auth_mode, wallet_address \
             FROM exchange_accounts WHERE id = $1 AND user_id = $2 AND auth_mode = $3",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(auth_modes::AGENT_WALLET)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?
        .ok_or(RepoError::NotFound)?;

        self.decrypt_credential_row(row)
    }

    /// Check if an agent wallet account is already active (approved).
    pub async fn is_agent_active(
        &self,
        account_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, RepoError> {
        let row: Option<(Option<bool>,)> = sqlx::query_as(
            "SELECT is_active FROM exchange_accounts \
             WHERE id = $1 AND user_id = $2 AND auth_mode = $3",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(auth_modes::AGENT_WALLET)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        match row {
            Some((active,)) => Ok(active.unwrap_or(false)),
            None => Err(RepoError::NotFound),
        }
    }

    /// Migrate an existing direct-key (api_key) account to agent-wallet mode.
    /// Replaces encrypted credentials with the new agent keypair, preserving the account ID.
    /// Sets `is_active = false` so the user must re-approve via the AW-02 flow.
    /// Uses `SELECT ... FOR UPDATE` to lock the row during keypair replacement,
    /// preventing concurrent approve/revoke from racing with the migration.
    pub async fn migrate_to_agent_wallet(
        &self,
        account_id: Uuid,
        user_id: Uuid,
        wallet_address: &str,
        agent_key: &str,
        agent_address: &str,
    ) -> Result<(), RepoError> {
        let encrypted_key = self
            .vault
            .encrypt(agent_address.as_bytes())
            .map_err(|e| RepoError::Encryption(e.to_string()))?;
        let encrypted_secret = self
            .vault
            .encrypt(agent_key.as_bytes())
            .map_err(|e| RepoError::Encryption(e.to_string()))?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        // Lock the row to prevent concurrent approve/revoke
        let locked = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM exchange_accounts \
             WHERE id = $1 AND user_id = $2 AND exchange_name = $3 AND auth_mode = $4 \
             FOR UPDATE",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(exchanges::HYPERLIQUID)
        .bind(auth_modes::API_KEY)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        if locked.is_none() {
            return Err(RepoError::NotFound);
        }

        // Perform the migration under the lock
        sqlx::query(
            "UPDATE exchange_accounts SET \
             api_key_encrypted = $3, api_secret_encrypted = $4, \
             auth_mode = $6, wallet_address = $5, \
             is_active = false, permissions = '{}'::jsonb \
             WHERE id = $1 AND user_id = $2",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(&encrypted_key)
        .bind(&encrypted_secret)
        .bind(wallet_address)
        .bind(auth_modes::AGENT_WALLET)
        .execute(&mut *tx)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(())
    }

    /// Revoke an agent wallet: deactivate and record revocation timestamp.
    /// Uses `WHERE is_active = true` precondition to prevent double-revocation.
    /// Returns `Ok(true)` on success, `Ok(false)` if precondition not met (already revoked).
    pub async fn revoke_agent(
        &self,
        account_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, RepoError> {
        let revoked_at = chrono::Utc::now().to_rfc3339();
        let permissions = serde_json::json!({
            "agent_approved": false,
            "revoked_at": revoked_at,
        });

        let result = sqlx::query_scalar::<_, Uuid>(
            "UPDATE exchange_accounts SET is_active = false, permissions = $3 \
             WHERE id = $1 AND user_id = $2 AND auth_mode = $4 AND is_active = true \
             RETURNING id",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(&permissions)
        .bind(auth_modes::AGENT_WALLET)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(result.is_some())
    }

    /// List active agent-wallet accounts approaching TTL expiry.
    /// Returns accounts where `updated_at` (or `created_at`) is older than `ttl_hours - 1` hours.
    pub async fn list_agent_wallets_approaching_ttl(
        &self,
        ttl_hours: u64,
    ) -> Result<Vec<ExchangeAccountRow>, RepoError> {
        let buffer_hours = (ttl_hours.saturating_sub(1)) as i64;
        sqlx::query_as::<_, ExchangeAccountRow>(
            "SELECT id, user_id, exchange_name, permissions, is_active, created_at, last_used_at, \
             auth_mode, wallet_address \
             FROM exchange_accounts \
             WHERE auth_mode = $2 AND is_active = true \
             AND created_at < NOW() - INTERVAL '1 hour' * $1 \
             ORDER BY created_at ASC",
        )
        .bind(buffer_hours)
        .bind(auth_modes::AGENT_WALLET)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))
    }

    /// Find existing agent wallet for a user+wallet_address combo.
    /// Returns active wallets first, then pending, most recent first.
    pub async fn find_agent_wallet(
        &self,
        user_id: Uuid,
        wallet_address: &str,
    ) -> Result<Option<ExchangeAccountRow>, RepoError> {
        sqlx::query_as::<_, ExchangeAccountRow>(
            "SELECT id, user_id, exchange_name, permissions, is_active, created_at, last_used_at, \
             auth_mode, wallet_address \
             FROM exchange_accounts \
             WHERE user_id = $1 AND wallet_address = $2 AND auth_mode = $3 \
             ORDER BY is_active DESC, created_at DESC \
             LIMIT 1",
        )
        .bind(user_id)
        .bind(wallet_address)
        .bind(auth_modes::AGENT_WALLET)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))
    }

    /// Atomically mark an agent wallet account as approved and active.
    /// Uses `WHERE is_active = false` precondition to prevent concurrent approvals.
    /// Returns `Ok(true)` on success, `Ok(false)` if precondition not met (already active).
    pub async fn update_agent_approved(
        &self,
        account_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, RepoError> {
        let result = sqlx::query_scalar::<_, Uuid>(
            "UPDATE exchange_accounts SET is_active = true, \
             permissions = jsonb_set(COALESCE(permissions, '{}'::jsonb), '{agent_approved}', 'true') \
             WHERE id = $1 AND user_id = $2 AND auth_mode = $3 AND is_active = false \
             RETURNING id",
        )
        .bind(account_id)
        .bind(user_id)
        .bind(auth_modes::AGENT_WALLET)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(result.is_some())
    }

    /// Get the last successfully synced fill timestamp for an exchange account.
    /// Returns `None` if the account has never been synced (first sync should pull 90 days).
    pub async fn get_last_synced_exec_time(
        &self,
        account_id: Uuid,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, RepoError> {
        let row: Option<(Option<chrono::DateTime<chrono::Utc>>,)> = sqlx::query_as(
            "SELECT last_synced_exec_time FROM exchange_accounts WHERE id = $1",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;

        Ok(row.and_then(|(ts,)| ts))
    }

    /// Advance the watermark to `ts` after a successful fill sync.
    pub async fn set_last_synced_exec_time(
        &self,
        account_id: Uuid,
        ts: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), RepoError> {
        sqlx::query(
            "UPDATE exchange_accounts SET last_synced_exec_time = $1 WHERE id = $2",
        )
        .bind(ts)
        .bind(account_id)
        .execute(&self.pool)
        .await
        .map_err(|e| RepoError::Database(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_encrypt_decrypt_roundtrip() {
        let key_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let vault = AesGcmVault::from_hex(key_hex).unwrap();

        let plaintext = b"my_secret_api_key_12345";
        let encrypted = vault.encrypt(plaintext).unwrap();

        assert!(encrypted.len() > plaintext.len());
        assert!(!encrypted.windows(plaintext.len()).any(|w| w == plaintext));

        let decrypted = vault.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_vault_different_nonces() {
        let key_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let vault = AesGcmVault::from_hex(key_hex).unwrap();

        let plaintext = b"same_plaintext";
        let enc1 = vault.encrypt(plaintext).unwrap();
        let enc2 = vault.encrypt(plaintext).unwrap();

        assert_ne!(enc1, enc2);
        assert_eq!(vault.decrypt(&enc1).unwrap(), plaintext);
        assert_eq!(vault.decrypt(&enc2).unwrap(), plaintext);
    }

    #[test]
    fn test_vault_invalid_key_length() {
        assert!(AesGcmVault::from_hex("0123456789abcdef").is_err());
    }

    #[test]
    fn test_vault_invalid_hex() {
        assert!(AesGcmVault::from_hex("not_hex_at_all_zzzz").is_err());
    }

    #[test]
    fn test_vault_decrypt_tampered_data() {
        let key_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let vault = AesGcmVault::from_hex(key_hex).unwrap();

        let encrypted = vault.encrypt(b"secret").unwrap();
        let mut tampered = encrypted.clone();
        if let Some(last) = tampered.last_mut() {
            *last ^= 0xff;
        }
        assert!(vault.decrypt(&tampered).is_err());
    }

    #[test]
    fn test_vault_decrypt_too_short() {
        let key_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let vault = AesGcmVault::from_hex(key_hex).unwrap();
        assert!(vault.decrypt(&[0u8; 5]).is_err());
    }

    #[test]
    fn test_repo_error_conflict_variant() {
        let err = RepoError::Conflict("already active".to_string());
        assert!(err.to_string().contains("already active"));
        assert!(matches!(err, RepoError::Conflict(_)));
    }

    #[test]
    fn test_repo_error_variants_are_distinct() {
        let not_found = RepoError::NotFound;
        let conflict = RepoError::Conflict("test".to_string());
        let db_error = RepoError::Database("test".to_string());

        // Verify pattern matching distinguishes all variants
        assert!(matches!(not_found, RepoError::NotFound));
        assert!(!matches!(not_found, RepoError::Conflict(_)));
        assert!(matches!(conflict, RepoError::Conflict(_)));
        assert!(!matches!(conflict, RepoError::NotFound));
        assert!(matches!(db_error, RepoError::Database(_)));
    }
}
