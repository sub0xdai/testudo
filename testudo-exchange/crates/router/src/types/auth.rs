use common_utils::models::User;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User information in API responses (AUTH-02: wallet-primary)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub wallet_address: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            wallet_address: user.wallet_address,
        }
    }
}

/// Response for successful logout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutResponse {
    pub message: String,
}

/// Generic message response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub message: String,
}

/// Generic error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        error: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
            details: Some(details),
        }
    }

    pub fn validation_error(details: serde_json::Value) -> Self {
        Self::with_details("validation_error", "Request validation failed", details)
    }

    pub fn unauthorized() -> Self {
        Self::new("unauthorized", "Authentication required")
    }

    pub fn forbidden() -> Self {
        Self::new("forbidden", "Access denied")
    }

    pub fn internal_error() -> Self {
        Self::new("internal_error", "Internal server error")
    }

    pub fn invalid_token() -> Self {
        Self::new("invalid_token", "Invalid or expired token")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_user_response_from_user() {
        let user = User {
            id: Uuid::new_v4(),
            wallet_address: "0xC285000000000000000000000000000000005b36".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            is_active: true,
            coach_enabled: true,
            coach_banner_last_viewed_at: None,
        };

        let response = UserResponse::from(user.clone());
        assert_eq!(response.id, user.id);
        assert_eq!(response.wallet_address, user.wallet_address);
    }

    #[test]
    fn test_error_response_constructors() {
        let basic_error = ErrorResponse::new("test_error", "Test message");
        assert_eq!(basic_error.error, "test_error");
        assert_eq!(basic_error.message, "Test message");
        assert!(basic_error.details.is_none());

        let detailed_error = ErrorResponse::with_details(
            "validation_error",
            "Validation failed",
            serde_json::json!({"field": "wallet_address"}),
        );
        assert_eq!(detailed_error.error, "validation_error");
        assert!(detailed_error.details.is_some());

        let unauthorized = ErrorResponse::unauthorized();
        assert_eq!(unauthorized.error, "unauthorized");

        let invalid_token = ErrorResponse::invalid_token();
        assert_eq!(invalid_token.error, "invalid_token");
    }
}
