/// Cryptographic utilities for secure API key management
///
/// This module provides secure encryption services for protecting sensitive
/// exchange API credentials using industry-standard AES-256-GCM encryption
/// with proper key derivation and authenticated encryption.
///
/// # Security Features
///
/// - **AES-256-GCM**: Authenticated encryption preventing tampering
/// - **PBKDF2**: Secure key derivation with configurable iterations
/// - **Random IV/Nonce**: Unique for each encryption operation
/// - **Memory Safety**: Automatic zeroization of sensitive data
/// - **Environment-based**: Master key loaded from environment variables
///
/// # Usage
///
/// ```rust
/// use common_utils::crypto::{EncryptionService, AesGcmVault};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create vault with master key from ENCRYPTION_MASTER_KEY env var
/// let vault = AesGcmVault::new()?;
///
/// // Encrypt sensitive API key
/// let api_key = "my_secret_api_key";
/// let encrypted = vault.encrypt(api_key).await?;
///
/// // Store encrypted data safely in database
/// // ...
///
/// // Later, decrypt when needed
/// let decrypted = vault.decrypt(&encrypted).await?;
/// assert_eq!(api_key, decrypted);
/// # Ok(())
/// # }
/// ```
pub mod errors;
pub mod vault;

// Re-export main types for convenience
pub use errors::EncryptionError;
pub use vault::{AesGcmVault, EncryptionService};

#[cfg(test)]
pub use vault::TestVault;
