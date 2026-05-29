// @anchor exchange:router:mod
// @tags api

pub mod nonce_store;
pub mod pairing_store;
pub mod siwe;
pub mod siws;

pub use nonce_store::NonceStore;
pub use pairing_store::PairingStore;
pub use siwe::{parse_siwe_message, recover_signer, validate_siwe_message, verify_siwe_signature, SiweMessage};
pub use siws::{parse_siws_message, validate_siws_message, verify_siws_signature, SiwsMessage};

use common_utils::auth::AuthError;

/// Normalize a wallet address to a canonical storage format.
///
/// - EVM: starts with "0x" and 42 chars total -> lowercase
/// - Solana: 32-44 chars, valid base58 -> preserve case
/// - Otherwise: error
pub fn normalize_wallet_address(addr: &str) -> Result<String, AuthError> {
    if addr.starts_with("0x") && addr.len() == 42 {
        Ok(addr.to_lowercase())
    } else if addr.len() >= 32 && addr.len() <= 44 {
        bs58::decode(addr)
            .into_vec()
            .map_err(|_| AuthError::Unauthorized("invalid wallet address".to_string()))?;
        Ok(addr.to_string())
    } else {
        Err(AuthError::Unauthorized(
            "unrecognized wallet address format".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_evm_address() {
        let addr = "0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36";
        let result = normalize_wallet_address(addr).unwrap();
        assert_eq!(result, "0xc2850eb36450ef3dd9ccc68f0eb38e575b365b36");
    }

    #[test]
    fn test_normalize_evm_already_lowercase() {
        let addr = "0xc2850eb36450ef3dd9ccc68f0eb38e575b365b36";
        let result = normalize_wallet_address(addr).unwrap();
        assert_eq!(result, addr);
    }

    #[test]
    fn test_normalize_solana_address() {
        let addr = "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU";
        let result = normalize_wallet_address(addr).unwrap();
        // Solana addresses preserve case
        assert_eq!(result, addr);
    }

    #[test]
    fn test_normalize_rejects_garbage() {
        let result = normalize_wallet_address("not-a-wallet");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_rejects_empty() {
        let result = normalize_wallet_address("");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_rejects_too_short() {
        let result = normalize_wallet_address("abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_rejects_0x_wrong_length() {
        let result = normalize_wallet_address("0x1234");
        assert!(result.is_err());
    }

    #[test]
    fn test_normalize_rejects_invalid_base58_in_solana_range() {
        // 'O' is not valid base58
        let result = normalize_wallet_address("OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO0");
        assert!(result.is_err());
    }
}
