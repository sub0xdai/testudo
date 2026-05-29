// @anchor exchange:common_utils:mod
// @tags infra

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// JWT token claims structure — wallet-primary (AUTH-02)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,            // user_id (UUID as string)
    pub wallet_address: String, // 0x-prefixed Ethereum address
    pub exp: i64,               // expiration timestamp
    pub iat: i64,               // issued at timestamp
    pub iss: String,            // token issuer URL (SEC-01)
    pub token_type: TokenType,
}

/// Token type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TokenType {
    Access,
    Refresh,
}

/// Authentication tokens pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64, // seconds until access token expires
}

/// Token lifetimes (AUTH-02 FR-5)
pub const ACCESS_TOKEN_EXPIRY_SECONDS: u64 = 900; // 15 minutes
pub const REFRESH_TOKEN_EXPIRY_SECONDS: u64 = 604_800; // 7 days

/// JWT token service for encoding/decoding tokens
pub trait TokenService: Send + Sync {
    fn generate_access_token(
        &self,
        user_id: &Uuid,
        wallet_address: &str,
    ) -> Result<String, AuthError>;
    fn generate_refresh_token(
        &self,
        user_id: &Uuid,
        wallet_address: &str,
    ) -> Result<String, AuthError>;
    fn verify_access_token(&self, token: &str) -> Result<TokenClaims, AuthError>;
    fn verify_refresh_token(&self, token: &str) -> Result<TokenClaims, AuthError>;
}

/// JWT implementation of TokenService
pub struct JwtTokenService {
    access_secret: String,
    refresh_secret: String,
    access_expires_in: Duration,
    refresh_expires_in: Duration,
}

impl JwtTokenService {
    pub fn new(access_secret: String, refresh_secret: String) -> Self {
        Self {
            access_secret,
            refresh_secret,
            access_expires_in: Duration::seconds(ACCESS_TOKEN_EXPIRY_SECONDS as i64),
            refresh_expires_in: Duration::seconds(REFRESH_TOKEN_EXPIRY_SECONDS as i64),
        }
    }

    fn encode_token(
        &self,
        user_id: &Uuid,
        wallet_address: &str,
        token_type: TokenType,
        expires_in: Duration,
        secret: &str,
    ) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = (now + expires_in).timestamp();

        let issuer = std::env::var("JWT_ISSUER")
            .unwrap_or_else(|_| "https://api.testudo.vip".to_string());

        let claims = TokenClaims {
            sub: user_id.to_string(),
            wallet_address: wallet_address.to_string(),
            exp,
            iat: now.timestamp(),
            iss: issuer,
            token_type,
        };

        let header = Header::default();
        let encoding_key = EncodingKey::from_secret(secret.as_ref());

        jsonwebtoken::encode(&header, &claims, &encoding_key)
            .map_err(|_| AuthError::TokenGenerationFailed)
    }

    fn decode_token(
        &self,
        token: &str,
        secret: &str,
        expected_type: TokenType,
    ) -> Result<TokenClaims, AuthError> {
        let decoding_key = DecodingKey::from_secret(secret.as_ref());
        let expected_issuer = std::env::var("JWT_ISSUER")
            .unwrap_or_else(|_| "https://api.testudo.vip".to_string());
        let mut validation = Validation::default();
        validation.set_issuer(&[&expected_issuer]);
        validation.validate_exp = true;

        let token_data = jsonwebtoken::decode::<TokenClaims>(token, &decoding_key, &validation)
            .map_err(|_| AuthError::InvalidToken)?;

        if token_data.claims.token_type != expected_type {
            return Err(AuthError::WrongTokenType);
        }

        // Check if token is expired
        let now = Utc::now().timestamp();
        if token_data.claims.exp < now {
            return Err(AuthError::TokenExpired);
        }

        Ok(token_data.claims)
    }
}

impl TokenService for JwtTokenService {
    fn generate_access_token(
        &self,
        user_id: &Uuid,
        wallet_address: &str,
    ) -> Result<String, AuthError> {
        self.encode_token(
            user_id,
            wallet_address,
            TokenType::Access,
            self.access_expires_in,
            &self.access_secret,
        )
    }

    fn generate_refresh_token(
        &self,
        user_id: &Uuid,
        wallet_address: &str,
    ) -> Result<String, AuthError> {
        self.encode_token(
            user_id,
            wallet_address,
            TokenType::Refresh,
            self.refresh_expires_in,
            &self.refresh_secret,
        )
    }

    fn verify_access_token(&self, token: &str) -> Result<TokenClaims, AuthError> {
        self.decode_token(token, &self.access_secret, TokenType::Access)
    }

    fn verify_refresh_token(&self, token: &str) -> Result<TokenClaims, AuthError> {
        self.decode_token(token, &self.refresh_secret, TokenType::Refresh)
    }
}

/// SHA-256 hash of a token for secure storage (AUTH-02 FR-6)
/// Used for refresh token hashing in user_sessions — fast lookup, not password verification.
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Authentication error types
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid token")]
    InvalidToken,
    #[error("Token has expired")]
    TokenExpired,
    #[error("Wrong token type")]
    WrongTokenType,
    #[error("Token generation failed")]
    TokenGenerationFailed,
    #[error("Session revoked")]
    SessionRevoked,
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Repository error: {0}")]
    RepositoryError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_generate_and_verify_jwt_tokens() {
        let token_service =
            JwtTokenService::new("access_secret".to_string(), "refresh_secret".to_string());

        let user_id = Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        // Generate access token
        let access_token = token_service
            .generate_access_token(&user_id, wallet)
            .unwrap();
        assert!(!access_token.is_empty());

        // Verify access token
        let claims = token_service.verify_access_token(&access_token).unwrap();
        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.wallet_address, wallet);
        assert_eq!(claims.token_type, TokenType::Access);

        // Generate refresh token
        let refresh_token = token_service
            .generate_refresh_token(&user_id, wallet)
            .unwrap();
        assert!(!refresh_token.is_empty());

        // Verify refresh token
        let refresh_claims = token_service.verify_refresh_token(&refresh_token).unwrap();
        assert_eq!(refresh_claims.sub, user_id.to_string());
        assert_eq!(refresh_claims.wallet_address, wallet);
        assert_eq!(refresh_claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn should_reject_wrong_token_type() {
        let token_service = JwtTokenService::new(
            "access_secret".to_string(),
            "access_secret".to_string(), // Same secret to ensure token decodes but has wrong type
        );

        let user_id = Uuid::new_v4();
        let access_token = token_service
            .generate_access_token(&user_id, "0xC285000000000000000000000000000000005b36")
            .unwrap();

        let result = token_service.verify_refresh_token(&access_token);
        assert!(result.is_err());
        if let Err(AuthError::WrongTokenType) = result {
            // Expected
        } else {
            panic!("Expected WrongTokenType error, got: {:?}", result);
        }
    }

    #[test]
    fn should_use_reduced_token_lifetimes() {
        let token_service =
            JwtTokenService::new("access_secret".to_string(), "refresh_secret".to_string());

        let user_id = Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        let access_token = token_service
            .generate_access_token(&user_id, wallet)
            .unwrap();
        let claims = token_service.verify_access_token(&access_token).unwrap();

        // Access token should expire in ~15 minutes (900 seconds)
        let ttl = claims.exp - claims.iat;
        assert_eq!(ttl, 900);

        let refresh_token = token_service
            .generate_refresh_token(&user_id, wallet)
            .unwrap();
        let refresh_claims = token_service.verify_refresh_token(&refresh_token).unwrap();

        // Refresh token should expire in ~7 days (604800 seconds)
        let refresh_ttl = refresh_claims.exp - refresh_claims.iat;
        assert_eq!(refresh_ttl, 604_800);
    }

    #[test]
    fn should_hash_token_deterministically() {
        let token = "test_refresh_token_value";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, token); // Hash should differ from input
        assert_eq!(hash1.len(), 64); // SHA-256 produces 64 hex chars
    }

    #[test]
    fn should_produce_different_hashes_for_different_tokens() {
        let hash1 = hash_token("token_a");
        let hash2 = hash_token("token_b");
        assert_ne!(hash1, hash2);
    }
}
