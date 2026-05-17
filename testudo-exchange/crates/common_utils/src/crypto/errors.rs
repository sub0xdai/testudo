/// Encryption error types for secure API key management
///
/// This module defines comprehensive error handling for the encryption service,
/// ensuring that cryptographic failures are properly categorized and never expose
/// sensitive information.
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EncryptionError {
    #[error("Failed to generate cryptographic key")]
    KeyGenerationFailed,

    #[error("Encryption operation failed")]
    EncryptionFailed,

    #[error("Decryption operation failed")]
    DecryptionFailed,

    #[error("Invalid ciphertext format")]
    InvalidCiphertext,

    #[error("Data tampering detected - authentication failed")]
    TamperedData,

    #[error("Missing encryption master key")]
    MissingMasterKey,

    #[error("Invalid key derivation parameters")]
    InvalidKeyDerivation,

    #[error("Insufficient entropy for secure operation")]
    InsufficientEntropy,
}

impl EncryptionError {
    /// Determines if the error indicates a security issue that should be logged
    pub fn is_security_critical(&self) -> bool {
        matches!(
            self,
            EncryptionError::TamperedData
                | EncryptionError::MissingMasterKey
                | EncryptionError::InsufficientEntropy
        )
    }

    /// Returns a user-safe error message that doesn't expose implementation details
    pub fn user_message(&self) -> &'static str {
        match self {
            EncryptionError::KeyGenerationFailed
            | EncryptionError::EncryptionFailed
            | EncryptionError::DecryptionFailed
            | EncryptionError::InvalidCiphertext
            | EncryptionError::TamperedData => "Encryption service temporarily unavailable",
            EncryptionError::MissingMasterKey => "System configuration error",
            EncryptionError::InvalidKeyDerivation | EncryptionError::InsufficientEntropy => {
                "Encryption service temporarily unavailable"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_identify_security_critical_errors() {
        assert!(EncryptionError::TamperedData.is_security_critical());
        assert!(EncryptionError::MissingMasterKey.is_security_critical());
        assert!(EncryptionError::InsufficientEntropy.is_security_critical());

        assert!(!EncryptionError::EncryptionFailed.is_security_critical());
        assert!(!EncryptionError::DecryptionFailed.is_security_critical());
        assert!(!EncryptionError::InvalidCiphertext.is_security_critical());
        assert!(!EncryptionError::KeyGenerationFailed.is_security_critical());
    }

    #[test]
    fn should_provide_safe_user_messages() {
        // All error messages should be safe for user consumption
        let errors = vec![
            EncryptionError::KeyGenerationFailed,
            EncryptionError::EncryptionFailed,
            EncryptionError::DecryptionFailed,
            EncryptionError::InvalidCiphertext,
            EncryptionError::TamperedData,
            EncryptionError::MissingMasterKey,
            EncryptionError::InvalidKeyDerivation,
            EncryptionError::InsufficientEntropy,
        ];

        for error in errors {
            let message = error.user_message();
            assert!(!message.is_empty());
            // Ensure no technical details leak
            assert!(!message.contains("key"));
            assert!(!message.contains("cipher"));
            assert!(!message.contains("crypto"));
            assert!(!message.contains("AES"));
            assert!(!message.contains("GCM"));
        }
    }

    #[test]
    fn should_format_errors_without_exposing_details() {
        let error = EncryptionError::TamperedData;
        let formatted = format!("{}", error);

        // Should contain meaningful information for developers
        assert!(formatted.contains("tampering"));

        // But shouldn't expose crypto implementation details
        assert!(!formatted.contains("AES"));
        assert!(!formatted.contains("GCM"));
        assert!(!formatted.contains("PBKDF2"));
    }
}
