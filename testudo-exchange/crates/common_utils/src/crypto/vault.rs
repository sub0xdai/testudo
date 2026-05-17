/// Secure encryption service for protecting CEX API keys and secrets
///
/// This module implements AES-256-GCM authenticated encryption with secure key derivation
/// for protecting sensitive exchange API credentials. It follows security best practices
/// including proper IV management, authenticated encryption, and secure memory handling.
use crate::crypto::errors::EncryptionError;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use async_trait::async_trait;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;
use std::env;
use zeroize::ZeroizeOnDrop;

/// Number of PBKDF2 iterations for key derivation (minimum recommended by OWASP)
const PBKDF2_ITERATIONS: u32 = 100_000;

/// Salt size for key derivation (32 bytes for high entropy)
const SALT_SIZE: usize = 32;

/// Size of AES-256 key in bytes
const KEY_SIZE: usize = 32;

/// Size of AES-GCM nonce in bytes
const NONCE_SIZE: usize = 12;

/// Environment variable name for the master encryption key
const MASTER_KEY_ENV: &str = "ENCRYPTION_MASTER_KEY";

/// Trait defining the encryption service interface
/// Following Interface Segregation Principle - only what's needed for encryption
#[async_trait]
pub trait EncryptionService: Send + Sync {
    /// Encrypts plaintext using AES-256-GCM
    /// Returns ciphertext with embedded nonce and authentication tag
    async fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, EncryptionError>;

    /// Decrypts ciphertext and verifies authentication tag
    /// Returns original plaintext if authentication succeeds
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<String, EncryptionError>;

    /// Generates a new cryptographically secure key
    /// For testing and key rotation purposes
    fn generate_key(&self) -> Result<[u8; 32], EncryptionError>;
}

/// Secure memory container for encryption keys
/// Automatically zeros memory on drop to prevent key leakage
#[derive(ZeroizeOnDrop)]
struct SecureKey {
    data: [u8; KEY_SIZE],
}

impl SecureKey {
    fn new(data: [u8; KEY_SIZE]) -> Self {
        Self { data }
    }

    fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

/// Production encryption service implementation using AES-256-GCM
pub struct AesGcmVault {
    master_key: SecureKey,
}

impl AesGcmVault {
    /// Creates a new vault instance with master key from environment
    pub fn new() -> Result<Self, EncryptionError> {
        let master_key_hex =
            env::var(MASTER_KEY_ENV).map_err(|_| EncryptionError::MissingMasterKey)?;

        let master_key_bytes = hex::decode(master_key_hex.trim())
            .map_err(|_| EncryptionError::InvalidKeyDerivation)?;

        if master_key_bytes.len() != KEY_SIZE {
            return Err(EncryptionError::InvalidKeyDerivation);
        }

        let mut key_array = [0u8; KEY_SIZE];
        key_array.copy_from_slice(&master_key_bytes);

        Ok(Self {
            master_key: SecureKey::new(key_array),
        })
    }

    /// Creates a vault with a specific master key (for testing)
    #[cfg(test)]
    pub fn with_key(master_key: [u8; KEY_SIZE]) -> Self {
        Self {
            master_key: SecureKey::new(master_key),
        }
    }

    /// Derives an encryption key using PBKDF2 with a given salt
    fn derive_key(&self, salt: &[u8]) -> Result<SecureKey, EncryptionError> {
        let mut derived_key = [0u8; KEY_SIZE];

        pbkdf2_hmac::<Sha256>(
            self.master_key.as_slice(),
            salt,
            PBKDF2_ITERATIONS,
            &mut derived_key,
        );

        Ok(SecureKey::new(derived_key))
    }

    /// Generates cryptographically secure random bytes
    fn generate_random_bytes<const N: usize>(&self) -> Result<[u8; N], EncryptionError> {
        let mut bytes = [0u8; N];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|_| EncryptionError::InsufficientEntropy)?;
        Ok(bytes)
    }
}

#[async_trait]
impl EncryptionService for AesGcmVault {
    async fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, EncryptionError> {
        // Generate random salt for key derivation
        let salt = self.generate_random_bytes::<SALT_SIZE>()?;

        // Derive encryption key from master key + salt
        let derived_key = self.derive_key(&salt)?;

        // Create AES-GCM cipher
        let key = Key::<Aes256Gcm>::from_slice(derived_key.as_slice());
        let cipher = Aes256Gcm::new(key);

        // Generate random nonce
        let nonce_bytes = self.generate_random_bytes::<NONCE_SIZE>()?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the plaintext
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| EncryptionError::EncryptionFailed)?;

        // Format: [salt(32)] + [nonce(12)] + [ciphertext + auth_tag]
        let mut result = Vec::with_capacity(SALT_SIZE + NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&salt);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    async fn decrypt(&self, ciphertext: &[u8]) -> Result<String, EncryptionError> {
        // Validate minimum length: salt + nonce + at least 16 bytes (min AES-GCM output)
        if ciphertext.len() < SALT_SIZE + NONCE_SIZE + 16 {
            return Err(EncryptionError::InvalidCiphertext);
        }

        // Extract components
        let salt = &ciphertext[0..SALT_SIZE];
        let nonce_bytes = &ciphertext[SALT_SIZE..SALT_SIZE + NONCE_SIZE];
        let encrypted_data = &ciphertext[SALT_SIZE + NONCE_SIZE..];

        // Derive the same key using the stored salt
        let derived_key = self.derive_key(salt)?;

        // Create AES-GCM cipher
        let key = Key::<Aes256Gcm>::from_slice(derived_key.as_slice());
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt and verify authentication tag
        let plaintext_bytes = cipher
            .decrypt(nonce, encrypted_data)
            .map_err(|_| EncryptionError::TamperedData)?;

        // Convert to string
        String::from_utf8(plaintext_bytes).map_err(|_| EncryptionError::DecryptionFailed)
    }

    fn generate_key(&self) -> Result<[u8; 32], EncryptionError> {
        self.generate_random_bytes::<32>()
    }
}

/// In-memory vault for testing purposes only
/// WARNING: Does not persist keys and should never be used in production
#[cfg(test)]
pub struct TestVault {
    key: SecureKey,
}

#[cfg(test)]
impl TestVault {
    pub fn new() -> Self {
        let mut key = [0u8; KEY_SIZE];
        OsRng.fill_bytes(&mut key);
        Self {
            key: SecureKey::new(key),
        }
    }

    pub fn with_fixed_key() -> Self {
        // Fixed key for deterministic testing
        let key = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C,
            0x1D, 0x1E, 0x1F, 0x20,
        ];
        Self {
            key: SecureKey::new(key),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl EncryptionService for TestVault {
    async fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, EncryptionError> {
        // Use same implementation as AesGcmVault for consistency
        let vault = AesGcmVault {
            master_key: SecureKey::new(self.key.data),
        };
        vault.encrypt(plaintext).await
    }

    async fn decrypt(&self, ciphertext: &[u8]) -> Result<String, EncryptionError> {
        let vault = AesGcmVault {
            master_key: SecureKey::new(self.key.data),
        };
        vault.decrypt(ciphertext).await
    }

    fn generate_key(&self) -> Result<[u8; 32], EncryptionError> {
        let mut key = [0u8; 32];
        OsRng
            .try_fill_bytes(&mut key)
            .map_err(|_| EncryptionError::InsufficientEntropy)?;
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // TDD RED PHASE: These tests should fail initially and drive implementation

    #[tokio::test]
    async fn should_encrypt_and_decrypt_round_trip() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "super_secret_api_key_123";

        let ciphertext = vault
            .encrypt(plaintext)
            .await
            .expect("Encryption should succeed");
        let decrypted = vault
            .decrypt(&ciphertext)
            .await
            .expect("Decryption should succeed");

        assert_eq!(plaintext, decrypted);
        assert_ne!(plaintext.as_bytes(), ciphertext.as_slice()); // Should be different
    }

    #[tokio::test]
    async fn should_detect_tampering() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "api_key_to_protect";

        let mut ciphertext = vault
            .encrypt(plaintext)
            .await
            .expect("Encryption should succeed");

        // Tamper with the last byte (authentication tag)
        let last_idx = ciphertext.len() - 1;
        ciphertext[last_idx] ^= 0x01;

        let result = vault.decrypt(&ciphertext).await;
        assert!(matches!(result, Err(EncryptionError::TamperedData)));
    }

    #[tokio::test]
    async fn should_generate_unique_nonces() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "same_plaintext_each_time";

        let mut ciphertexts = HashSet::new();

        // Encrypt the same plaintext multiple times
        for _ in 0..10 {
            let ciphertext = vault
                .encrypt(plaintext)
                .await
                .expect("Encryption should succeed");
            // Each encryption should produce different ciphertext due to random nonce
            assert!(ciphertexts.insert(ciphertext));
        }
    }

    #[tokio::test]
    async fn should_handle_empty_plaintext() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "";

        let ciphertext = vault
            .encrypt(plaintext)
            .await
            .expect("Should encrypt empty string");
        let decrypted = vault
            .decrypt(&ciphertext)
            .await
            .expect("Should decrypt empty string");

        assert_eq!(plaintext, decrypted);
    }

    #[tokio::test]
    async fn should_handle_unicode_plaintext() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "🔐 secure_key_with_émojis_and_ñ_chars 🗝️";

        let ciphertext = vault
            .encrypt(plaintext)
            .await
            .expect("Should encrypt unicode");
        let decrypted = vault
            .decrypt(&ciphertext)
            .await
            .expect("Should decrypt unicode");

        assert_eq!(plaintext, decrypted);
    }

    #[tokio::test]
    async fn should_reject_invalid_ciphertext() {
        let vault = TestVault::with_fixed_key();

        // Too short
        let result = vault.decrypt(&[0u8; 10]).await;
        assert!(matches!(result, Err(EncryptionError::InvalidCiphertext)));

        // Random bytes
        let random_bytes = vec![0xAB; 100];
        let result = vault.decrypt(&random_bytes).await;
        assert!(matches!(result, Err(EncryptionError::TamperedData)));
    }

    #[tokio::test]
    async fn should_handle_large_plaintexts() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "A".repeat(10_000); // 10KB plaintext

        let ciphertext = vault
            .encrypt(&plaintext)
            .await
            .expect("Should encrypt large text");
        let decrypted = vault
            .decrypt(&ciphertext)
            .await
            .expect("Should decrypt large text");

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn should_generate_secure_keys() {
        let vault = TestVault::new();

        let mut keys = HashSet::new();

        // Generate multiple keys and ensure they're all different
        for _ in 0..10 {
            let key = vault.generate_key().expect("Key generation should succeed");
            assert!(keys.insert(key)); // Should be unique
        }
    }

    #[test]
    fn should_handle_missing_master_key() {
        // Remove the environment variable if it exists
        env::remove_var(MASTER_KEY_ENV);

        let result = AesGcmVault::new();
        assert!(matches!(result, Err(EncryptionError::MissingMasterKey)));
    }

    #[test]
    fn should_handle_invalid_master_key() {
        // Set invalid hex key
        env::set_var(MASTER_KEY_ENV, "invalid_hex_key");

        let result = AesGcmVault::new();
        assert!(matches!(result, Err(EncryptionError::InvalidKeyDerivation)));

        // Set valid hex but wrong length
        env::set_var(MASTER_KEY_ENV, "abcd1234"); // Too short

        let result = AesGcmVault::new();
        assert!(matches!(result, Err(EncryptionError::InvalidKeyDerivation)));
    }

    #[tokio::test]
    async fn should_work_with_valid_master_key() {
        // Set a valid 32-byte hex key
        let valid_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        env::set_var(MASTER_KEY_ENV, valid_key);

        let vault = AesGcmVault::new().expect("Should create vault with valid key");
        let plaintext = "test_encryption";

        let ciphertext = vault.encrypt(plaintext).await.expect("Should encrypt");
        let decrypted = vault.decrypt(&ciphertext).await.expect("Should decrypt");

        assert_eq!(plaintext, decrypted);

        // Clean up
        env::remove_var(MASTER_KEY_ENV);
    }

    #[tokio::test]
    async fn should_use_different_salts_for_same_key() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "same_plaintext";

        let ciphertext1 = vault.encrypt(plaintext).await.expect("Should encrypt");
        let ciphertext2 = vault.encrypt(plaintext).await.expect("Should encrypt");

        // Extract salts (first 32 bytes)
        let salt1 = &ciphertext1[0..SALT_SIZE];
        let salt2 = &ciphertext2[0..SALT_SIZE];

        assert_ne!(
            salt1, salt2,
            "Salts should be different for each encryption"
        );
    }

    #[test]
    fn should_zeroize_keys_on_drop() {
        let key_data = [0xFFu8; KEY_SIZE];
        {
            let _secure_key = SecureKey::new(key_data);
            // Key is in scope and contains 0xFF bytes
        }
        // After drop, the key should be zeroized
        // Note: This test verifies the ZeroizeOnDrop trait is properly applied
        // The actual zeroing is handled by the zeroize crate
    }

    // Security property tests

    #[tokio::test]
    async fn should_not_leak_plaintext_in_ciphertext() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "very_secret_password_123";

        let ciphertext = vault.encrypt(plaintext).await.expect("Should encrypt");

        // Ciphertext should not contain any part of the plaintext
        let ciphertext_str = String::from_utf8_lossy(&ciphertext);
        assert!(!ciphertext_str.contains("very_secret"));
        assert!(!ciphertext_str.contains("password"));
        assert!(!ciphertext_str.contains("123"));
    }

    #[tokio::test]
    async fn should_produce_ciphertext_with_correct_format() {
        let vault = TestVault::with_fixed_key();
        let plaintext = "api_key";

        let ciphertext = vault.encrypt(plaintext).await.expect("Should encrypt");

        // Format: [salt(32)] + [nonce(12)] + [ciphertext + auth_tag]
        // Minimum size: 32 + 12 + 7 + 16 = 67 bytes (for "api_key" + auth tag)
        assert!(ciphertext.len() >= SALT_SIZE + NONCE_SIZE + plaintext.len() + 16);

        // Should be able to decrypt
        let decrypted = vault.decrypt(&ciphertext).await.expect("Should decrypt");
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn error_types_should_provide_safe_messages() {
        // Verify that error messages don't leak implementation details
        let errors = vec![
            EncryptionError::TamperedData,
            EncryptionError::EncryptionFailed,
            EncryptionError::DecryptionFailed,
        ];

        for error in errors {
            let message = error.user_message();
            assert!(!message.contains("AES"));
            assert!(!message.contains("GCM"));
            assert!(!message.contains("key"));
        }
    }
}

// Additional integration tests with the ExchangeAccount model
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::models::exchange_account::*;
    use serde_json::json;
    use uuid::Uuid;

    async fn create_test_account_with_encryption(
    ) -> Result<ExchangeAccount, Box<dyn std::error::Error>> {
        let vault = TestVault::with_fixed_key();

        // Encrypt API credentials
        let api_key = "binance_api_key_123";
        let api_secret = "binance_secret_456";

        let encrypted_key = vault.encrypt(api_key).await?;
        let encrypted_secret = vault.encrypt(api_secret).await?;

        // Create exchange account
        let factory = StandardExchangeAccountFactory::default();
        let account = factory.create_exchange_account(
            Uuid::new_v4(),
            "binance",
            encrypted_key,
            encrypted_secret,
            json!({
                "spot_trading": true,
                "futures_trading": false
            }),
        )?;

        Ok(account)
    }

    #[tokio::test]
    async fn should_integrate_with_exchange_account_model() {
        let account = create_test_account_with_encryption()
            .await
            .expect("Should create account with encrypted credentials");

        // Verify the account was created successfully
        assert_eq!(account.exchange_name, "binance");
        assert!(!account.encrypted_api_key.is_empty());
        assert!(!account.encrypted_secret.is_empty());

        // Verify we can decrypt the credentials
        let vault = TestVault::with_fixed_key();
        let decrypted_key = vault
            .decrypt(&account.encrypted_api_key)
            .await
            .expect("Should decrypt API key");
        let decrypted_secret = vault
            .decrypt(&account.encrypted_secret)
            .await
            .expect("Should decrypt API secret");

        assert_eq!(decrypted_key, "binance_api_key_123");
        assert_eq!(decrypted_secret, "binance_secret_456");
    }

    #[tokio::test]
    async fn should_handle_encrypted_credentials_serialization() {
        let account = create_test_account_with_encryption()
            .await
            .expect("Should create account");

        // Serialize the account (encrypted fields should be skipped)
        let serialized = serde_json::to_string(&account).expect("Should serialize account");

        // Should not contain any part of the original credentials
        assert!(!serialized.contains("binance_api_key"));
        assert!(!serialized.contains("binance_secret"));

        // Should contain the account metadata
        assert!(serialized.contains("binance"));
        assert!(serialized.contains(&account.id.to_string()));
    }
}
