// @anchor exchange:router:siws
// @tags api

use chrono::{DateTime, Utc};
use common_utils::auth::AuthError;
use ed25519_dalek::{Signature, VerifyingKey};

/// Parsed Sign-In with Solana (SIWS) message.
#[derive(Debug, Clone)]
pub struct SiwsMessage {
    pub domain: String,
    pub address: String,
    pub statement: Option<String>,
    pub uri: String,
    pub nonce: String,
    pub issued_at: DateTime<Utc>,
    pub expiration_time: Option<DateTime<Utc>>,
}

/// Parse a SIWS plaintext message into structured fields.
///
/// SIWS format:
/// ```text
/// {domain} wants you to sign in with your Solana account:
/// {address}
///
/// {statement (optional)}
///
/// URI: {uri}
/// Nonce: {nonce}
/// Issued At: {issued_at}
/// Expiration Time: {expiration_time} (optional)
/// ```
pub fn parse_siws_message(message: &str) -> Result<SiwsMessage, AuthError> {
    let lines: Vec<&str> = message.lines().collect();

    if lines.len() < 5 {
        return Err(AuthError::Unauthorized(
            "SIWS message too short".to_string(),
        ));
    }

    // Line 0: "{domain} wants you to sign in with your Solana account:"
    let domain = lines[0]
        .strip_suffix(" wants you to sign in with your Solana account:")
        .ok_or_else(|| AuthError::Unauthorized("invalid SIWS header".to_string()))?
        .to_string();

    if domain.is_empty() {
        return Err(AuthError::Unauthorized("empty domain".to_string()));
    }

    // Line 1: base58 Solana public key
    let address = lines[1].trim().to_string();
    validate_solana_address(&address)?;

    // Find the URI line — everything between address and URI is the optional statement
    let uri_idx = lines
        .iter()
        .position(|line| line.starts_with("URI: "))
        .ok_or_else(|| AuthError::Unauthorized("missing URI field".to_string()))?;

    // Statement: lines between address (line 1) and URI line, trimmed of empty lines
    let statement_text = lines[2..uri_idx]
        .to_vec()
        .join("\n")
        .trim()
        .to_string();
    let statement = if statement_text.is_empty() {
        None
    } else {
        Some(statement_text)
    };

    // Parse required fields from URI line onwards
    let field_lines = &lines[uri_idx..];
    let uri = extract_field(field_lines, "URI: ")?;
    let nonce = extract_field(field_lines, "Nonce: ")?;
    let issued_at_str = extract_field(field_lines, "Issued At: ")?;
    let issued_at = DateTime::parse_from_rfc3339(&issued_at_str)
        .map_err(|_| AuthError::Unauthorized("invalid Issued At timestamp".to_string()))?
        .with_timezone(&Utc);

    // Optional fields
    let expiration_time = extract_optional_field(field_lines, "Expiration Time: ")
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(SiwsMessage {
        domain,
        address,
        statement,
        uri,
        nonce,
        issued_at,
        expiration_time,
    })
}

/// Validate SIWS message fields against expected values.
pub fn validate_siws_message(
    msg: &SiwsMessage,
    expected_domain: &str,
    nonce_valid: bool,
) -> Result<(), AuthError> {
    if msg.domain != expected_domain {
        return Err(AuthError::Unauthorized(format!(
            "domain mismatch: expected {}, got {}",
            expected_domain, msg.domain
        )));
    }

    if !nonce_valid {
        return Err(AuthError::Unauthorized(
            "invalid or expired nonce".to_string(),
        ));
    }

    if let Some(exp) = msg.expiration_time {
        if Utc::now() > exp {
            return Err(AuthError::TokenExpired);
        }
    }

    Ok(())
}

/// Verify an Ed25519 signature over the raw message bytes.
///
/// Solana wallets sign the message bytes directly (no hashing preamble like EIP-191).
/// Returns the claimed address on success.
pub fn verify_siws_signature(
    message: &str,
    signature_b58: &str,
    claimed_address: &str,
) -> Result<String, AuthError> {
    let pubkey_bytes = bs58::decode(claimed_address)
        .into_vec()
        .map_err(|_| AuthError::Unauthorized("invalid base58 address".to_string()))?;

    let pubkey_array: [u8; 32] = pubkey_bytes
        .try_into()
        .map_err(|_| AuthError::Unauthorized("pubkey must be 32 bytes".to_string()))?;

    let verifying_key = VerifyingKey::from_bytes(&pubkey_array)
        .map_err(|e| AuthError::Unauthorized(format!("invalid pubkey: {e}")))?;

    let sig_bytes = bs58::decode(signature_b58)
        .into_vec()
        .map_err(|_| AuthError::Unauthorized("invalid base58 signature".to_string()))?;

    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| AuthError::Unauthorized("signature must be 64 bytes".to_string()))?;

    let signature = Signature::from_bytes(&sig_array);

    // Use verify() not verify_strict() — Phantom and other Solana wallets
    // may produce signatures that fail strict verification (cofactor checks).
    use ed25519_dalek::Verifier;
    verifying_key
        .verify(message.as_bytes(), &signature)
        .map_err(|_| AuthError::Unauthorized("Ed25519 signature verification failed".to_string()))?;

    Ok(claimed_address.to_string())
}

/// Validate that a string is a plausible base58-encoded Solana public key.
fn validate_solana_address(address: &str) -> Result<(), AuthError> {
    if address.len() < 32 || address.len() > 44 {
        return Err(AuthError::Unauthorized(format!(
            "invalid Solana address length: {}",
            address.len()
        )));
    }

    let decoded = bs58::decode(address)
        .into_vec()
        .map_err(|_| AuthError::Unauthorized("invalid base58 in Solana address".to_string()))?;

    if decoded.len() != 32 {
        return Err(AuthError::Unauthorized(format!(
            "Solana pubkey must decode to 32 bytes, got {}",
            decoded.len()
        )));
    }

    Ok(())
}

fn extract_field(lines: &[&str], prefix: &str) -> Result<String, AuthError> {
    lines
        .iter()
        .find_map(|line| line.strip_prefix(prefix).map(String::from))
        .ok_or_else(|| {
            AuthError::Unauthorized(format!(
                "missing {} field",
                prefix.trim_end_matches(": ")
            ))
        })
}

fn extract_optional_field(lines: &[&str], prefix: &str) -> Option<String> {
    lines
        .iter()
        .find_map(|line| line.strip_prefix(prefix).map(String::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::Rng;

    fn make_keypair() -> (SigningKey, String) {
        let mut secret_bytes = [0u8; 32];
        rand::thread_rng().fill(&mut secret_bytes);
        let signing_key = SigningKey::from_bytes(&secret_bytes);
        let verifying_key = signing_key.verifying_key();
        let address = bs58::encode(verifying_key.as_bytes()).into_string();
        (signing_key, address)
    }

    fn sample_siws_message() -> String {
        "testudo.app wants you to sign in with your Solana account:\n\
         7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU\n\
         \n\
         Sign in to Testudo\n\
         \n\
         URI: https://testudo.app\n\
         Nonce: abc123def456\n\
         Issued At: 2026-04-09T12:00:00Z"
            .to_string()
    }

    fn sample_siws_message_no_statement() -> String {
        "testudo.app wants you to sign in with your Solana account:\n\
         7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU\n\
         \n\
         URI: https://testudo.app\n\
         Nonce: abc123def456\n\
         Issued At: 2026-04-09T12:00:00Z"
            .to_string()
    }

    fn sample_siws_message_with_expiry() -> String {
        "testudo.app wants you to sign in with your Solana account:\n\
         7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU\n\
         \n\
         Sign in to Testudo\n\
         \n\
         URI: https://testudo.app\n\
         Nonce: abc123def456\n\
         Issued At: 2026-04-09T12:00:00Z\n\
         Expiration Time: 2099-12-31T23:59:59Z"
            .to_string()
    }

    // -----------------------------------------------------------------------
    // Parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_basic_message() {
        let msg = parse_siws_message(&sample_siws_message()).unwrap();
        assert_eq!(msg.domain, "testudo.app");
        assert_eq!(msg.address, "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU");
        assert_eq!(msg.statement, Some("Sign in to Testudo".to_string()));
        assert_eq!(msg.uri, "https://testudo.app");
        assert_eq!(msg.nonce, "abc123def456");
        assert!(msg.expiration_time.is_none());
    }

    #[test]
    fn test_parse_no_statement() {
        let msg = parse_siws_message(&sample_siws_message_no_statement()).unwrap();
        assert_eq!(msg.domain, "testudo.app");
        assert!(msg.statement.is_none());
        assert_eq!(msg.nonce, "abc123def456");
    }

    #[test]
    fn test_parse_with_expiration() {
        let msg = parse_siws_message(&sample_siws_message_with_expiry()).unwrap();
        assert!(msg.expiration_time.is_some());
    }

    #[test]
    fn test_parse_too_short() {
        let result = parse_siws_message("too short");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty() {
        let result = parse_siws_message("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_header() {
        let msg = "evil.com wants you to sign in with your Ethereum account:\n\
                   7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU\n\
                   \n\
                   URI: https://testudo.app\n\
                   Nonce: abc123\n\
                   Issued At: 2026-04-09T12:00:00Z";
        let result = parse_siws_message(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_nonce() {
        let msg = "testudo.app wants you to sign in with your Solana account:\n\
                   7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU\n\
                   \n\
                   URI: https://testudo.app\n\
                   Issued At: 2026-04-09T12:00:00Z";
        let result = parse_siws_message(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_address_too_short() {
        let msg = "testudo.app wants you to sign in with your Solana account:\n\
                   abc\n\
                   \n\
                   URI: https://testudo.app\n\
                   Nonce: abc123\n\
                   Issued At: 2026-04-09T12:00:00Z";
        let result = parse_siws_message(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_address_bad_base58() {
        // 'O' and 'I' are not valid base58 characters
        let msg = "testudo.app wants you to sign in with your Solana account:\n\
                   OOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOOO0\n\
                   \n\
                   URI: https://testudo.app\n\
                   Nonce: abc123\n\
                   Issued At: 2026-04-09T12:00:00Z";
        let result = parse_siws_message(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_uri() {
        let msg = "testudo.app wants you to sign in with your Solana account:\n\
                   7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU\n\
                   \n\
                   Nonce: abc123\n\
                   Issued At: 2026-04-09T12:00:00Z";
        let result = parse_siws_message(msg);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_domain_match() {
        let msg = parse_siws_message(&sample_siws_message()).unwrap();
        assert!(validate_siws_message(&msg, "testudo.app", true).is_ok());
    }

    #[test]
    fn test_validate_domain_mismatch() {
        let msg = parse_siws_message(&sample_siws_message()).unwrap();
        assert!(validate_siws_message(&msg, "evil.com", true).is_err());
    }

    #[test]
    fn test_validate_invalid_nonce() {
        let msg = parse_siws_message(&sample_siws_message()).unwrap();
        assert!(validate_siws_message(&msg, "testudo.app", false).is_err());
    }

    #[test]
    fn test_validate_expired_message() {
        let expired_msg = "testudo.app wants you to sign in with your Solana account:\n\
                           7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU\n\
                           \n\
                           URI: https://testudo.app\n\
                           Nonce: abc123\n\
                           Issued At: 2020-01-01T00:00:00Z\n\
                           Expiration Time: 2020-01-02T00:00:00Z";
        let msg = parse_siws_message(expired_msg).unwrap();
        assert!(validate_siws_message(&msg, "testudo.app", true).is_err());
    }

    #[test]
    fn test_validate_not_expired() {
        let msg = parse_siws_message(&sample_siws_message_with_expiry()).unwrap();
        // Expiry is 2099 -- should pass
        assert!(validate_siws_message(&msg, "testudo.app", true).is_ok());
    }

    #[test]
    fn test_validate_localhost_domain() {
        let msg_str = "localhost:3001 wants you to sign in with your Solana account:\n\
                       7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU\n\
                       \n\
                       URI: http://localhost:3001\n\
                       Nonce: abc123\n\
                       Issued At: 2026-04-09T12:00:00Z";
        let msg = parse_siws_message(msg_str).unwrap();
        assert_eq!(msg.domain, "localhost:3001");
        assert!(validate_siws_message(&msg, "localhost:3001", true).is_ok());
        assert!(validate_siws_message(&msg, "testudo.app", true).is_err());
    }

    // -----------------------------------------------------------------------
    // Ed25519 signature verification tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_real_ed25519_signature() {
        let (signing_key, address) = make_keypair();
        let message = "test message for signing";
        let signature = signing_key.sign(message.as_bytes());
        let sig_b58 = bs58::encode(signature.to_bytes()).into_string();

        let result = verify_siws_signature(message, &sig_b58, &address).unwrap();
        assert_eq!(result, address);
    }

    #[test]
    fn test_verify_rejects_tampered_signature() {
        let (signing_key, address) = make_keypair();
        let message = "test message for signing";
        let signature = signing_key.sign(message.as_bytes());
        let mut sig_bytes = signature.to_bytes();
        sig_bytes[0] ^= 0xFF; // flip bits
        let tampered_sig = bs58::encode(sig_bytes).into_string();

        let result = verify_siws_signature(message, &tampered_sig, &address);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_rejects_wrong_message() {
        let (signing_key, address) = make_keypair();
        let signature = signing_key.sign(b"original message");
        let sig_b58 = bs58::encode(signature.to_bytes()).into_string();

        let result = verify_siws_signature("different message", &sig_b58, &address);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_rejects_wrong_address() {
        let (signing_key, _address) = make_keypair();
        let (_other_key, other_address) = make_keypair();
        let message = "test message";
        let signature = signing_key.sign(message.as_bytes());
        let sig_b58 = bs58::encode(signature.to_bytes()).into_string();

        let result = verify_siws_signature(message, &sig_b58, &other_address);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_rejects_invalid_base58_signature() {
        let (_signing_key, address) = make_keypair();
        let result = verify_siws_signature("msg", "not-valid-base58!!!", &address);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_rejects_short_signature() {
        let (_signing_key, address) = make_keypair();
        let result = verify_siws_signature("msg", "3QJmV3qfvL9", &address); // too short
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_rejects_invalid_base58_address() {
        let result = verify_siws_signature("msg", "3QJmV3qfvL9", "not-valid!!!");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Full round-trip: build SIWS message, sign, verify
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_siws_round_trip() {
        let (signing_key, address) = make_keypair();

        let message = format!(
            "testudo.app wants you to sign in with your Solana account:\n\
             {}\n\
             \n\
             Sign in to Testudo\n\
             \n\
             URI: https://testudo.app\n\
             Nonce: testnonce123\n\
             Issued At: 2026-04-09T12:00:00Z",
            address
        );

        // Sign the raw message bytes (same as Phantom signMessage)
        let signature = signing_key.sign(message.as_bytes());
        let sig_b58 = bs58::encode(signature.to_bytes()).into_string();

        // Parse
        let parsed = parse_siws_message(&message).unwrap();
        assert_eq!(parsed.domain, "testudo.app");
        assert_eq!(parsed.address, address);
        assert_eq!(parsed.statement, Some("Sign in to Testudo".to_string()));

        // Validate
        assert!(validate_siws_message(&parsed, "testudo.app", true).is_ok());

        // Verify signature
        let recovered = verify_siws_signature(&message, &sig_b58, &address).unwrap();
        assert_eq!(recovered, address);
    }

    #[test]
    fn test_full_siws_round_trip_no_statement() {
        let (signing_key, address) = make_keypair();

        let message = format!(
            "testudo.app wants you to sign in with your Solana account:\n\
             {}\n\
             \n\
             URI: https://testudo.app\n\
             Nonce: testnonce123\n\
             Issued At: 2026-04-09T12:00:00Z",
            address
        );

        let signature = signing_key.sign(message.as_bytes());
        let sig_b58 = bs58::encode(signature.to_bytes()).into_string();

        let parsed = parse_siws_message(&message).unwrap();
        assert!(parsed.statement.is_none());

        let recovered = verify_siws_signature(&message, &sig_b58, &address).unwrap();
        assert_eq!(recovered, address);
    }
}
