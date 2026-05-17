use alloy::primitives::{eip191_hash_message, Address, Signature};
use chrono::{DateTime, Utc};
use common_utils::auth::AuthError;

/// Parsed EIP-4361 (Sign-In with Ethereum) message.
#[derive(Debug, Clone)]
pub struct SiweMessage {
    pub domain: String,
    pub address: Address,
    pub statement: Option<String>,
    pub uri: String,
    pub version: String,
    pub chain_id: u64,
    pub nonce: String,
    pub issued_at: DateTime<Utc>,
    pub expiration_time: Option<DateTime<Utc>>,
}

/// Parse an EIP-4361 plaintext message into structured fields.
///
/// EIP-4361 format:
/// ```text
/// {domain} wants you to sign in with your Ethereum account:
/// {address}
///
/// {statement (optional)}
///
/// URI: {uri}
/// Version: {version}
/// Chain ID: {chain_id}
/// Nonce: {nonce}
/// Issued At: {issued_at}
/// Expiration Time: {expiration_time} (optional)
/// ```
pub fn parse_siwe_message(message: &str) -> Result<SiweMessage, AuthError> {
    let lines: Vec<&str> = message.lines().collect();

    if lines.len() < 7 {
        return Err(AuthError::Unauthorized(
            "SIWE message too short".to_string(),
        ));
    }

    // Line 0: "{domain} wants you to sign in with your Ethereum account:"
    let domain = lines[0]
        .strip_suffix(" wants you to sign in with your Ethereum account:")
        .ok_or_else(|| AuthError::Unauthorized("invalid SIWE header".to_string()))?
        .to_string();

    if domain.is_empty() {
        return Err(AuthError::Unauthorized("empty domain".to_string()));
    }

    // Line 1: 0x-prefixed Ethereum address
    let address: Address = lines[1]
        .parse()
        .map_err(|_| AuthError::Unauthorized("invalid Ethereum address".to_string()))?;

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
    let version = extract_field(field_lines, "Version: ")?;
    let chain_id: u64 = extract_field(field_lines, "Chain ID: ")?
        .parse()
        .map_err(|_| AuthError::Unauthorized("invalid chain ID".to_string()))?;
    let nonce = extract_field(field_lines, "Nonce: ")?;
    let issued_at_str = extract_field(field_lines, "Issued At: ")?;
    let issued_at = DateTime::parse_from_rfc3339(&issued_at_str)
        .map_err(|_| AuthError::Unauthorized("invalid Issued At timestamp".to_string()))?
        .with_timezone(&Utc);

    // Optional fields
    let expiration_time = extract_optional_field(field_lines, "Expiration Time: ")
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(SiweMessage {
        domain,
        address,
        statement,
        uri,
        version,
        chain_id,
        nonce,
        issued_at,
        expiration_time,
    })
}

/// Validate SIWE message fields against expected values.
pub fn validate_siwe_message(
    msg: &SiweMessage,
    expected_domain: &str,
    nonce_valid: bool,
) -> Result<(), AuthError> {
    if msg.domain != expected_domain {
        return Err(AuthError::Unauthorized(format!(
            "domain mismatch: expected {}, got {}",
            expected_domain, msg.domain
        )));
    }

    if msg.version != "1" {
        return Err(AuthError::Unauthorized(format!(
            "unsupported SIWE version: {}",
            msg.version
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

/// Recover the Ethereum address that signed the given message using EIP-191 personal sign.
pub fn recover_signer(message: &str, signature_hex: &str) -> Result<Address, AuthError> {
    let hash = eip191_hash_message(message.as_bytes());

    // Parse 65-byte signature from hex (with or without 0x prefix)
    let sig_hex = signature_hex.strip_prefix("0x").unwrap_or(signature_hex);
    let sig_bytes = hex::decode(sig_hex)
        .map_err(|e| AuthError::Unauthorized(format!("invalid signature hex: {}", e)))?;

    if sig_bytes.len() != 65 {
        return Err(AuthError::Unauthorized(format!(
            "signature must be 65 bytes, got {}",
            sig_bytes.len()
        )));
    }

    // Split into r (32) + s (32) + v (1)
    let mut r_s = [0u8; 64];
    r_s.copy_from_slice(&sig_bytes[..64]);
    let v = sig_bytes[64];

    // Normalize v: Ethereum uses 27/28, alloy expects 0/1 parity
    let parity = match v {
        0 | 1 => v == 1,
        27 | 28 => v == 28,
        _ => {
            return Err(AuthError::Unauthorized(format!(
                "invalid signature v value: {}",
                v
            )))
        }
    };

    let sig = Signature::from_bytes_and_parity(&r_s, parity)
        .map_err(|e| AuthError::Unauthorized(format!("invalid signature: {}", e)))?;
    sig.recover_address_from_prehash(&hash)
        .map_err(|e| AuthError::Unauthorized(format!("signature recovery failed: {}", e)))
}

/// Verify that a SIWE signature was created by the address claimed in the message.
pub fn verify_siwe_signature(message: &str, signature_hex: &str) -> Result<Address, AuthError> {
    let parsed = parse_siwe_message(message)?;
    let recovered = recover_signer(message, signature_hex)?;

    if recovered != parsed.address {
        return Err(AuthError::Unauthorized(format!(
            "signature mismatch: message claims {}, recovered {}",
            parsed.address, recovered
        )));
    }

    Ok(recovered)
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

    fn sample_siwe_message() -> String {
        "testudo.app wants you to sign in with your Ethereum account:\n\
         0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36\n\
         \n\
         Sign in to Testudo\n\
         \n\
         URI: https://testudo.app\n\
         Version: 1\n\
         Chain ID: 1\n\
         Nonce: abc123def456\n\
         Issued At: 2026-03-24T12:00:00Z"
            .to_string()
    }

    fn sample_siwe_message_no_statement() -> String {
        "testudo.app wants you to sign in with your Ethereum account:\n\
         0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36\n\
         \n\
         URI: https://testudo.app\n\
         Version: 1\n\
         Chain ID: 1\n\
         Nonce: abc123def456\n\
         Issued At: 2026-03-24T12:00:00Z"
            .to_string()
    }

    fn sample_siwe_message_with_expiry() -> String {
        "testudo.app wants you to sign in with your Ethereum account:\n\
         0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36\n\
         \n\
         Sign in to Testudo\n\
         \n\
         URI: https://testudo.app\n\
         Version: 1\n\
         Chain ID: 1\n\
         Nonce: abc123def456\n\
         Issued At: 2026-03-24T12:00:00Z\n\
         Expiration Time: 2099-12-31T23:59:59Z"
            .to_string()
    }

    #[test]
    fn test_parse_basic_message() {
        let msg = parse_siwe_message(&sample_siwe_message()).unwrap();
        assert_eq!(msg.domain, "testudo.app");
        assert_eq!(
            msg.address,
            "0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36"
                .parse::<Address>()
                .unwrap()
        );
        assert_eq!(msg.statement, Some("Sign in to Testudo".to_string()));
        assert_eq!(msg.uri, "https://testudo.app");
        assert_eq!(msg.version, "1");
        assert_eq!(msg.chain_id, 1);
        assert_eq!(msg.nonce, "abc123def456");
        assert!(msg.expiration_time.is_none());
    }

    #[test]
    fn test_parse_no_statement() {
        let msg = parse_siwe_message(&sample_siwe_message_no_statement()).unwrap();
        assert_eq!(msg.domain, "testudo.app");
        assert!(msg.statement.is_none());
        assert_eq!(msg.nonce, "abc123def456");
    }

    #[test]
    fn test_parse_with_expiration() {
        let msg = parse_siwe_message(&sample_siwe_message_with_expiry()).unwrap();
        assert!(msg.expiration_time.is_some());
    }

    #[test]
    fn test_parse_invalid_header() {
        let result = parse_siwe_message("not a valid SIWE message");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_message() {
        let result = parse_siwe_message("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_nonce() {
        let msg = "testudo.app wants you to sign in with your Ethereum account:\n\
                   0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36\n\
                   \n\
                   URI: https://testudo.app\n\
                   Version: 1\n\
                   Chain ID: 1\n\
                   Issued At: 2026-03-24T12:00:00Z";
        let result = parse_siwe_message(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_address() {
        let msg = "testudo.app wants you to sign in with your Ethereum account:\n\
                   not-an-address\n\
                   \n\
                   URI: https://testudo.app\n\
                   Version: 1\n\
                   Chain ID: 1\n\
                   Nonce: abc123\n\
                   Issued At: 2026-03-24T12:00:00Z";
        let result = parse_siwe_message(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_domain_match() {
        let msg = parse_siwe_message(&sample_siwe_message()).unwrap();
        assert!(validate_siwe_message(&msg, "testudo.app", true).is_ok());
    }

    #[test]
    fn test_validate_domain_mismatch() {
        let msg = parse_siwe_message(&sample_siwe_message()).unwrap();
        assert!(validate_siwe_message(&msg, "evil.com", true).is_err());
    }

    #[test]
    fn test_validate_invalid_nonce() {
        let msg = parse_siwe_message(&sample_siwe_message()).unwrap();
        assert!(validate_siwe_message(&msg, "testudo.app", false).is_err());
    }

    #[test]
    fn test_validate_expired_message() {
        let expired_msg = "testudo.app wants you to sign in with your Ethereum account:\n\
                           0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36\n\
                           \n\
                           URI: https://testudo.app\n\
                           Version: 1\n\
                           Chain ID: 1\n\
                           Nonce: abc123\n\
                           Issued At: 2020-01-01T00:00:00Z\n\
                           Expiration Time: 2020-01-02T00:00:00Z";
        let msg = parse_siwe_message(expired_msg).unwrap();
        assert!(validate_siwe_message(&msg, "testudo.app", true).is_err());
    }

    #[test]
    fn test_validate_unsupported_version() {
        let msg = "testudo.app wants you to sign in with your Ethereum account:\n\
                   0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36\n\
                   \n\
                   URI: https://testudo.app\n\
                   Version: 2\n\
                   Chain ID: 1\n\
                   Nonce: abc123\n\
                   Issued At: 2026-03-24T12:00:00Z";
        let parsed = parse_siwe_message(msg).unwrap();
        assert!(validate_siwe_message(&parsed, "testudo.app", true).is_err());
    }

    #[test]
    fn test_recover_signer_invalid_hex() {
        let result = recover_signer("hello", "0xnotvalidhex");
        assert!(result.is_err());
    }

    #[test]
    fn test_recover_signer_wrong_length() {
        let result = recover_signer("hello", "0xaabb");
        assert!(result.is_err());
    }

    #[test]
    fn test_recover_signer_invalid_v() {
        // 64 bytes of zeros + invalid v=99
        let sig = format!("0x{}{:02x}", "00".repeat(64), 99);
        let result = recover_signer("hello", &sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_siwe_signature_mismatch() {
        // A valid-format signature that recovers to a different address
        let msg = &sample_siwe_message();
        // Random 65-byte signature (v=27, will recover to some random address)
        let sig = format!("0x{}1b", "ab".repeat(64));
        // This should either fail recovery or detect address mismatch
        let result = verify_siwe_signature(msg, &sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_chain_id_values() {
        for (chain_id_str, expected) in &[("1", 1u64), ("42161", 42161), ("421614", 421614)] {
            let msg = format!(
                "testudo.app wants you to sign in with your Ethereum account:\n\
                 0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36\n\
                 \n\
                 URI: https://testudo.app\n\
                 Version: 1\n\
                 Chain ID: {}\n\
                 Nonce: abc123\n\
                 Issued At: 2026-03-24T12:00:00Z",
                chain_id_str
            );
            let parsed = parse_siwe_message(&msg).unwrap();
            assert_eq!(parsed.chain_id, *expected);
        }
    }

    #[test]
    fn test_recover_signer_with_real_key() {
        // Use alloy to generate a real key, sign, then verify recovery
        use alloy::signers::local::PrivateKeySigner;
        use alloy::signers::Signer;

        let signer = PrivateKeySigner::random();
        let address = signer.address();
        let message = "test message for signing";

        // Sign using EIP-191 personal sign
        let rt = tokio::runtime::Runtime::new().unwrap();
        let signature = rt.block_on(async { signer.sign_message(message.as_bytes()).await.unwrap() });

        let sig_hex = format!("0x{}", hex::encode(signature.as_bytes()));
        let recovered = recover_signer(message, &sig_hex).unwrap();
        assert_eq!(recovered, address);
    }

    #[test]
    fn test_full_siwe_flow_with_real_key() {
        use alloy::signers::local::PrivateKeySigner;
        use alloy::signers::Signer;

        let signer = PrivateKeySigner::random();
        let address = signer.address();

        let message = format!(
            "testudo.app wants you to sign in with your Ethereum account:\n\
             {}\n\
             \n\
             Sign in to Testudo\n\
             \n\
             URI: https://testudo.app\n\
             Version: 1\n\
             Chain ID: 1\n\
             Nonce: testnonce123\n\
             Issued At: 2026-03-24T12:00:00Z",
            address
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let signature = rt.block_on(async { signer.sign_message(message.as_bytes()).await.unwrap() });

        let sig_hex = format!("0x{}", hex::encode(signature.as_bytes()));

        // Full verification: parse + recover + compare
        let recovered = verify_siwe_signature(&message, &sig_hex).unwrap();
        assert_eq!(recovered, address);
    }

    // =========================================================================
    // REGRESSION: Address formatting — alloy {:#} truncates to ellipsis
    // Bug: format!("{address:#}") → "0xc285…5b36" (13 chars, NOT 42)
    // Fix: format!("{address}") → "0xC285F922..." (42 chars, full hex)
    // =========================================================================

    #[test]
    fn test_address_display_is_full_42_chars() {
        let addr: Address = "0xC285F922116959Db9eAF9f07729faBB7370A5b36"
            .parse()
            .unwrap();

        // Standard display: full 42-char checksummed hex
        let standard = format!("{addr}");
        assert_eq!(standard.len(), 42, "standard display must be 42 chars, got {}: {}", standard.len(), standard);
        assert!(standard.starts_with("0x"), "must start with 0x: {}", standard);

        // Alternate display: TRUNCATED — must NEVER be used for storage
        let alternate = format!("{addr:#}");
        assert!(alternate.len() < 42, "alternate display should be truncated: {}", alternate);
        assert!(alternate.contains('…'), "alternate display should contain ellipsis: {}", alternate);
    }

    #[test]
    fn test_address_lowercase_is_valid_wallet_format() {
        let addr: Address = "0xC285F922116959Db9eAF9f07729faBB7370A5b36"
            .parse()
            .unwrap();

        // This is the exact format we store in the DB
        let wallet_str = format!("{addr}").to_lowercase();

        assert_eq!(wallet_str.len(), 42);
        assert!(wallet_str.starts_with("0x"));
        // Must match the DB constraint: ^0x[0-9a-f]{40}$
        let re = regex::Regex::new(r"^0x[0-9a-f]{40}$").unwrap();
        assert!(re.is_match(&wallet_str), "wallet_str doesn't match DB constraint: {}", wallet_str);
    }

    #[test]
    fn test_recovered_address_matches_db_format() {
        use alloy::signers::local::PrivateKeySigner;
        use alloy::signers::Signer;

        let signer = PrivateKeySigner::random();
        let address = signer.address();

        let message = format!(
            "localhost:3001 wants you to sign in with your Ethereum account:\n\
             {}\n\
             \n\
             Sign in to Testudo\n\
             \n\
             URI: http://localhost:3001\n\
             Version: 1\n\
             Chain ID: 42161\n\
             Nonce: testnonce123\n\
             Issued At: 2026-03-24T12:00:00Z",
            address
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let signature = rt.block_on(async { signer.sign_message(message.as_bytes()).await.unwrap() });
        let sig_hex = format!("0x{}", hex::encode(signature.as_bytes()));

        let recovered = verify_siwe_signature(&message, &sig_hex).unwrap();

        // This is EXACTLY what the verify-siwe handler does:
        let wallet_str = format!("{recovered}").to_lowercase();

        // Must be valid for DB constraint
        assert_eq!(wallet_str.len(), 42);
        let re = regex::Regex::new(r"^0x[0-9a-f]{40}$").unwrap();
        assert!(re.is_match(&wallet_str),
            "recovered address doesn't match DB constraint: {} (len={})",
            wallet_str, wallet_str.len());
    }

    #[test]
    fn test_alternate_display_must_not_be_used_for_storage() {
        use alloy::signers::local::PrivateKeySigner;

        let signer = PrivateKeySigner::random();
        let address = signer.address();

        let bad_format = format!("{address:#}").to_lowercase();
        let good_format = format!("{address}").to_lowercase();

        // The bad format contains an ellipsis and is too short
        assert!(bad_format.contains('…') || bad_format.len() < 42,
            "alternate format should be truncated or contain ellipsis");

        // The good format is exactly 42 chars of hex
        assert_eq!(good_format.len(), 42);
        assert!(!good_format.contains('…'));
    }

    // =========================================================================
    // REGRESSION: Domain validation must work with localhost:port
    // Bug: SIWE_DOMAIN defaulted to "testudo.app" but dev sends "localhost:3001"
    // =========================================================================

    #[test]
    fn test_validate_localhost_domain() {
        let msg_str = "localhost:3001 wants you to sign in with your Ethereum account:\n\
                       0xC2850Eb36450EF3dd9cCC68f0eb38e575b365b36\n\
                       \n\
                       Sign in to Testudo\n\
                       \n\
                       URI: http://localhost:3001\n\
                       Version: 1\n\
                       Chain ID: 42161\n\
                       Nonce: abc123\n\
                       Issued At: 2026-03-24T12:00:00Z";
        let msg = parse_siwe_message(msg_str).unwrap();
        assert_eq!(msg.domain, "localhost:3001");
        assert!(validate_siwe_message(&msg, "localhost:3001", true).is_ok());
        assert!(validate_siwe_message(&msg, "testudo.app", true).is_err());
    }

    #[test]
    fn test_validate_production_domain() {
        let msg = parse_siwe_message(&sample_siwe_message()).unwrap();
        assert!(validate_siwe_message(&msg, "testudo.app", true).is_ok());
        assert!(validate_siwe_message(&msg, "localhost:3001", true).is_err());
    }
}
