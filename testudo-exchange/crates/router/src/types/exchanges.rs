// @anchor exchange:router:exchanges
// @tags api

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

/// Request to add a new exchange account
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ExchangeAccountRequest {
    /// Name of the exchange (e.g., "binance", "coinbase")
    #[validate(length(min = 1, max = 50))]
    pub exchange_name: String,

    /// Optional custom name for this account
    #[validate(length(max = 100))]
    pub account_name: Option<String>,

    /// API key for the exchange
    #[validate(length(min = 1, max = 500))]
    pub api_key: String,

    /// API secret for the exchange
    #[validate(length(min = 1, max = 500))]
    pub secret: String,

    /// Optional passphrase (required for some exchanges like Coinbase Pro)
    #[validate(length(max = 100))]
    pub passphrase: Option<String>,

    /// Optional permissions configuration
    pub permissions: Option<Value>,
}

/// Response containing exchange account information (without sensitive data)
#[derive(Debug, Serialize, Deserialize)]
pub struct ExchangeAccountResponse {
    /// Account ID
    pub id: Uuid,

    /// Exchange name
    pub exchange_name: String,

    /// Custom account name
    pub account_name: String,

    /// Whether the account is active
    pub is_active: bool,

    /// Account permissions (trading, reading, etc.)
    pub permissions: Value,

    /// When the account was created
    pub created_at: DateTime<Utc>,

    /// When the account was last used
    pub last_used_at: Option<DateTime<Utc>>,

    /// Authentication mode: "api_key" (default) or "agent_wallet"
    pub auth_mode: String,

    /// User's main wallet address (only for agent_wallet accounts)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_address: Option<String>,

    /// True when agent wallet is inactive and needs re-authorization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_reauthorization: Option<bool>,
}

/// Response for listing available exchanges
#[derive(Debug, Serialize, Deserialize)]
pub struct ExchangeListResponse {
    /// List of supported exchanges with their metadata
    pub exchanges: Vec<Value>,
}

/// Response for testing exchange connection
#[derive(Debug, Serialize, Deserialize)]
pub struct TestConnectionResponse {
    /// Account ID that was tested
    pub account_id: Uuid,

    /// Exchange name
    pub exchange_name: String,

    /// Connection status ("success", "failed", "warning")
    pub status: String,

    /// Human-readable message about the connection test
    pub message: String,

    /// When the test was performed
    pub tested_at: DateTime<Utc>,

    /// Optional latency measurement in milliseconds
    pub latency_ms: Option<u64>,

    /// Optional API rate limit information
    pub api_limits: Option<Value>,
}

/// Single asset balance entry for exchange balance response
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExchangeBalanceEntry {
    /// Asset symbol (e.g., "USDT", "BTC")
    pub asset: String,

    /// Total balance (free + used)
    pub total: String,

    /// Available (free) balance
    pub free: String,

    /// Locked (used) balance
    pub used: String,
}

/// Response for fetching exchange account balance
#[derive(Debug, Serialize, Deserialize)]
pub struct ExchangeBalanceResponse {
    /// Account ID
    pub account_id: Uuid,

    /// Exchange name
    pub exchange_name: String,

    /// All non-zero asset balances
    pub balances: Vec<ExchangeBalanceEntry>,

    /// When the balance was fetched
    pub fetched_at: DateTime<Utc>,
}

/// Request to initialize an agent wallet for Hyperliquid.
#[derive(Debug, Serialize, Deserialize)]
pub struct InitAgentWalletRequest {
    /// User's main wallet address (0x-prefixed, 42 chars)
    pub wallet_address: String,
}

/// Response from agent wallet initialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct InitAgentWalletResponse {
    /// The created exchange account ID
    pub account_id: Uuid,
    /// The agent's derived ETH address
    pub agent_address: String,
}

/// Request to get EIP-712 typed data for agent wallet approval.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveDataRequest {
    /// The agent wallet account ID (from init_agent_wallet)
    pub account_id: Uuid,
}

/// Response containing EIP-712 typed data for frontend signing.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveDataResponse {
    /// Full EIP-712 JSON for eth_signTypedData_v4
    pub typed_data: Value,
    /// Millisecond timestamp nonce used in the typed data
    pub nonce: u64,
    /// Agent address for frontend display
    pub agent_address: String,
}

/// Request to submit a signed agent approval to Hyperliquid.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveAgentRequest {
    /// The agent wallet account ID
    pub account_id: Uuid,
    /// 0x-prefixed hex signature (65 bytes: r + s + v)
    pub signature: String,
    /// Must match the nonce from approve-data
    pub nonce: u64,
}

/// Response from agent approval submission.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveAgentResponse {
    /// Whether the approval was successful
    pub success: bool,
    /// The approved agent address
    pub agent_address: String,
    /// Human-readable status message
    pub message: String,
}

/// Request to migrate an existing direct-key account to agent-wallet mode.
#[derive(Debug, Serialize, Deserialize)]
pub struct MigrateToAgentWalletRequest {
    /// ID of the existing exchange account to migrate
    pub account_id: Uuid,
    /// User's main wallet address (0x-prefixed, 42 chars)
    pub wallet_address: String,
}

/// Response from agent wallet migration.
#[derive(Debug, Serialize, Deserialize)]
pub struct MigrateToAgentWalletResponse {
    /// The existing account ID (preserved — no broken references)
    pub account_id: Uuid,
    /// The newly generated agent's ETH address
    pub agent_address: String,
    /// Instruction for the user
    pub message: String,
}

/// Response from agent wallet revocation.
#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeAgentResponse {
    /// Whether the revocation was successful
    pub success: bool,
    /// Human-readable status message
    pub message: String,
}

/// Request to update exchange account settings
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateExchangeAccountRequest {
    /// Optional new account name
    #[validate(length(max = 100))]
    pub account_name: Option<String>,

    /// Optional permissions update
    pub permissions: Option<Value>,

    /// Whether to activate/deactivate the account
    pub is_active: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exchange_account_request_validation() {
        // Valid request
        let valid = ExchangeAccountRequest {
            exchange_name: "binance".to_string(),
            account_name: Some("My Binance".to_string()),
            api_key: "valid_api_key".to_string(),
            secret: "valid_secret".to_string(),
            passphrase: None,
            permissions: Some(serde_json::json!({"spot": true})),
        };
        assert!(valid.validate().is_ok());

        // Invalid - empty exchange name
        let invalid = ExchangeAccountRequest {
            exchange_name: "".to_string(),
            account_name: Some("My Account".to_string()),
            api_key: "valid_api_key".to_string(),
            secret: "valid_secret".to_string(),
            passphrase: None,
            permissions: None,
        };
        assert!(invalid.validate().is_err());

        // Invalid - empty API key
        let invalid = ExchangeAccountRequest {
            exchange_name: "binance".to_string(),
            account_name: Some("My Account".to_string()),
            api_key: "".to_string(),
            secret: "valid_secret".to_string(),
            passphrase: None,
            permissions: None,
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_exchange_account_response_serialization() {
        let response = ExchangeAccountResponse {
            id: Uuid::new_v4(),
            exchange_name: "binance".to_string(),
            account_name: "Test Account".to_string(),
            is_active: true,
            permissions: serde_json::json!({"spot": true, "futures": false}),
            created_at: Utc::now(),
            last_used_at: Some(Utc::now()),
            auth_mode: "api_key".to_string(),
            wallet_address: None,
            requires_reauthorization: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("binance"));
        assert!(json.contains("Test Account"));

        let deserialized: ExchangeAccountResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.exchange_name, "binance");
        assert_eq!(deserialized.account_name, "Test Account");
    }

    #[test]
    fn test_test_connection_response() {
        let response = TestConnectionResponse {
            account_id: Uuid::new_v4(),
            exchange_name: "coinbase".to_string(),
            status: "success".to_string(),
            message: "Connection successful".to_string(),
            tested_at: Utc::now(),
            latency_ms: Some(123),
            api_limits: Some(serde_json::json!({
                "requests_per_second": 10,
                "burst_capacity": 100
            })),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("coinbase"));
        assert!(json.contains("success"));
        assert!(json.contains("123"));
    }

    #[test]
    fn test_update_exchange_account_request() {
        let update = UpdateExchangeAccountRequest {
            account_name: Some("Updated Name".to_string()),
            permissions: Some(serde_json::json!({"spot": false, "futures": true})),
            is_active: Some(false),
        };

        assert!(update.validate().is_ok());

        // Test with very long account name
        let invalid = UpdateExchangeAccountRequest {
            account_name: Some("a".repeat(101)), // Too long
            permissions: None,
            is_active: None,
        };

        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_init_agent_wallet_request_roundtrip() {
        let req = InitAgentWalletRequest {
            wallet_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: InitAgentWalletRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.wallet_address, req.wallet_address);
    }

    #[test]
    fn test_init_agent_wallet_response_roundtrip() {
        let resp = InitAgentWalletResponse {
            account_id: Uuid::new_v4(),
            agent_address: "0xabcdef1234567890abcdef1234567890abcdef12".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: InitAgentWalletResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.account_id, resp.account_id);
        assert_eq!(deserialized.agent_address, resp.agent_address);
    }

    #[test]
    fn test_approve_data_request_roundtrip() {
        let req = ApproveDataRequest {
            account_id: Uuid::new_v4(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: ApproveDataRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.account_id, req.account_id);
    }

    #[test]
    fn test_approve_data_response_serialization() {
        let resp = ApproveDataResponse {
            typed_data: serde_json::json!({"types": {}, "domain": {}}),
            nonce: 1710600000000,
            agent_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("typed_data"));
        assert!(json.contains("1710600000000"));
        assert!(json.contains("agent_address"));
    }

    #[test]
    fn test_approve_agent_request_roundtrip() {
        let req = ApproveAgentRequest {
            account_id: Uuid::new_v4(),
            signature: "0xaabb".to_string(),
            nonce: 1710600000000,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: ApproveAgentRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.account_id, req.account_id);
        assert_eq!(deserialized.signature, req.signature);
        assert_eq!(deserialized.nonce, req.nonce);
    }

    #[test]
    fn test_approve_agent_response_serialization() {
        let resp = ApproveAgentResponse {
            success: true,
            agent_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
            message: "Agent approved".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("true"));
        assert!(json.contains("agent_address"));
        assert!(json.contains("Agent approved"));
    }

    #[test]
    fn test_migrate_to_agent_wallet_request_roundtrip() {
        let req = MigrateToAgentWalletRequest {
            account_id: Uuid::new_v4(),
            wallet_address: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: MigrateToAgentWalletRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.account_id, req.account_id);
        assert_eq!(deserialized.wallet_address, req.wallet_address);
    }

    #[test]
    fn test_migrate_to_agent_wallet_response_roundtrip() {
        let resp = MigrateToAgentWalletResponse {
            account_id: Uuid::new_v4(),
            agent_address: "0xabcdef1234567890abcdef1234567890abcdef12".to_string(),
            message: "Agent keypair generated. Please approve via wallet.".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: MigrateToAgentWalletResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.account_id, resp.account_id);
        assert_eq!(deserialized.agent_address, resp.agent_address);
    }

    #[test]
    fn test_revoke_agent_response_roundtrip() {
        let resp = RevokeAgentResponse {
            success: true,
            message: "Agent wallet revoked".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: RevokeAgentResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.message, "Agent wallet revoked");
    }
}
