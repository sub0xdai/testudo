//! Credential Validator Module
//!
//! Validates exchange API credentials before storing them.
//! Calls the exchange API to verify authentication and permissions.

use super::ccxt_auth::CCXTAuthenticator;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Default Binance API base URL
const BINANCE_API_URL: &str = "https://api.binance.com";

/// Validated permissions returned after successful credential validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatedPermissions {
    pub can_trade_spot: bool,
    pub can_read_balances: bool,
    pub account_type: String,
}

/// Errors that can occur during credential validation
#[derive(Debug, Clone, PartialEq)]
pub enum CredentialValidationError {
    /// API key or secret is invalid
    InvalidCredentials,
    /// Credentials valid but missing required permissions
    InsufficientPermissions { missing: Vec<String> },
    /// Could not reach the exchange
    ExchangeUnreachable(String),
    /// Hit exchange rate limit
    RateLimited { retry_after_ms: u64 },
}

impl fmt::Display for CredentialValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCredentials => write!(f, "API key or secret is invalid"),
            Self::InsufficientPermissions { missing } => {
                write!(f, "Missing required permissions: {}", missing.join(", "))
            }
            Self::ExchangeUnreachable(msg) => write!(f, "Could not reach exchange: {}", msg),
            Self::RateLimited { retry_after_ms } => {
                write!(f, "Rate limited, retry after {}ms", retry_after_ms)
            }
        }
    }
}

impl std::error::Error for CredentialValidationError {}

/// Binance account response structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BinanceAccountResponse {
    can_trade: bool,
    account_type: String,
    #[serde(default)]
    balances: Vec<BinanceBalance>,
}

#[derive(Debug, Deserialize)]
struct BinanceBalance {
    #[allow(dead_code)]
    asset: String,
    #[allow(dead_code)]
    free: String,
    #[allow(dead_code)]
    locked: String,
}

/// Validates exchange API credentials by calling the exchange API
pub struct CredentialValidator {
    http_client: reqwest::Client,
    binance_base_url: String,
}

impl CredentialValidator {
    /// Creates a new credential validator with default Binance URL
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
            binance_base_url: BINANCE_API_URL.to_string(),
        }
    }

    /// Creates a validator with a custom Binance base URL (for testing)
    pub fn with_binance_url(base_url: &str) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            binance_base_url: base_url.to_string(),
        }
    }

    /// Validates Binance API credentials
    ///
    /// Calls GET /api/v3/account to verify:
    /// 1. Credentials are valid (authentication succeeds)
    /// 2. Account has required permissions (canTrade, balances accessible)
    pub async fn validate_binance(
        &self,
        api_key: &str,
        api_secret: &str,
    ) -> Result<ValidatedPermissions, CredentialValidationError> {
        let auth = CCXTAuthenticator::binance(api_key.to_string(), api_secret.to_string())
            .map_err(|e| CredentialValidationError::ExchangeUnreachable(e.to_string()))?;

        // Sign the request - this adds timestamp and signature
        let signed_query = auth
            .sign_binance_request(&[])
            .map_err(|e| CredentialValidationError::ExchangeUnreachable(e.to_string()))?;

        let url = format!("{}/api/v3/account?{}", self.binance_base_url, signed_query);

        let response = self
            .http_client
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .await
            .map_err(|e| CredentialValidationError::ExchangeUnreachable(e.to_string()))?;

        let status = response.status();

        // Handle rate limiting
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60000);
            return Err(CredentialValidationError::RateLimited {
                retry_after_ms: retry_after,
            });
        }

        // Handle authentication errors
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(CredentialValidationError::InvalidCredentials);
        }

        if !status.is_success() {
            return Err(CredentialValidationError::ExchangeUnreachable(format!(
                "Unexpected status: {}",
                status
            )));
        }

        let account: BinanceAccountResponse = response
            .json()
            .await
            .map_err(|e| CredentialValidationError::ExchangeUnreachable(e.to_string()))?;

        // Check required permissions
        let can_read_balances = !account.balances.is_empty() || account.can_trade;

        Ok(ValidatedPermissions {
            can_trade_spot: account.can_trade,
            can_read_balances,
            account_type: account.account_type,
        })
    }
}

impl Default for CredentialValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Binance account response with trading enabled
    fn binance_account_response_trading_enabled() -> serde_json::Value {
        serde_json::json!({
            "makerCommission": 10,
            "takerCommission": 10,
            "buyerCommission": 0,
            "sellerCommission": 0,
            "canTrade": true,
            "canWithdraw": false,
            "canDeposit": true,
            "updateTime": 1234567890,
            "accountType": "SPOT",
            "balances": [
                {"asset": "BTC", "free": "0.001", "locked": "0.0"},
                {"asset": "USDT", "free": "100.0", "locked": "0.0"}
            ],
            "permissions": ["SPOT"]
        })
    }

    #[tokio::test]
    async fn validate_binance_returns_permissions_on_valid_credentials() {
        // Arrange: Set up mock Binance API server
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex("/api/v3/account.*"))
            .and(header("X-MBX-APIKEY", "valid_api_key"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(binance_account_response_trading_enabled()),
            )
            .mount(&mock_server)
            .await;

        // Act: Validate credentials using mock server
        let validator = CredentialValidator::with_binance_url(&mock_server.uri());
        let result = validator
            .validate_binance("valid_api_key", "valid_secret")
            .await;

        // Assert: Should return valid permissions
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        let permissions = result.unwrap();
        assert!(permissions.can_trade_spot);
        assert!(permissions.can_read_balances);
        assert_eq!(permissions.account_type, "SPOT");
    }

    #[tokio::test]
    async fn validate_binance_returns_invalid_credentials_on_401() {
        // Arrange
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex("/api/v3/account.*"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "code": -2015,
                "msg": "Invalid API-key, IP, or permissions for action."
            })))
            .mount(&mock_server)
            .await;

        // Act
        let validator = CredentialValidator::with_binance_url(&mock_server.uri());
        let result = validator
            .validate_binance("invalid_key", "invalid_secret")
            .await;

        // Assert
        assert!(matches!(
            result,
            Err(CredentialValidationError::InvalidCredentials)
        ));
    }

    #[tokio::test]
    async fn validate_binance_returns_rate_limited_on_429() {
        // Arrange
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex("/api/v3/account.*"))
            .respond_with(
                ResponseTemplate::new(429)
                    .insert_header("Retry-After", "30000")
                    .set_body_json(serde_json::json!({
                        "code": -1015,
                        "msg": "Too many requests"
                    })),
            )
            .mount(&mock_server)
            .await;

        // Act
        let validator = CredentialValidator::with_binance_url(&mock_server.uri());
        let result = validator.validate_binance("some_key", "some_secret").await;

        // Assert
        match result {
            Err(CredentialValidationError::RateLimited { retry_after_ms }) => {
                assert_eq!(retry_after_ms, 30000);
            }
            other => panic!("Expected RateLimited, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn validate_binance_detects_no_trading_permission() {
        // Arrange: Account without canTrade permission
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path_regex("/api/v3/account.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "makerCommission": 10,
                "takerCommission": 10,
                "canTrade": false,  // No trading permission
                "canWithdraw": false,
                "canDeposit": true,
                "accountType": "SPOT",
                "balances": []
            })))
            .mount(&mock_server)
            .await;

        // Act
        let validator = CredentialValidator::with_binance_url(&mock_server.uri());
        let result = validator
            .validate_binance("read_only_key", "read_only_secret")
            .await;

        // Assert: Should succeed but report no trading permission
        assert!(result.is_ok());
        let permissions = result.unwrap();
        assert!(!permissions.can_trade_spot); // Cannot trade
    }
}
