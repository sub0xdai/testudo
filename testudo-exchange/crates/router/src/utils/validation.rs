//! β002: Universal validation abstractions that make parsing errors impossible
//!
//! This module implements semantic compression by creating validated types
//! that eliminate the need for repeated UUID parsing and validation across routes.

// @anchor exchange:router:validation
// @tags api

use actix_web::error::{ErrorBadRequest, ErrorForbidden};
use actix_web::Error as ActixError;
use std::str::FromStr;
use uuid::Uuid;

use crate::middleware::AuthenticatedUser;
use crate::types::auth::ErrorResponse;
use crate::types::exchange_names::exchanges;

/// Validated UUID wrapper that guarantees valid UUID at compile time
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedUuid(Uuid);

impl ValidatedUuid {
    /// Parse and validate UUID from string, returning Actix error on failure
    pub fn parse(input: &str) -> Result<Self, ActixError> {
        Uuid::from_str(input)
            .map(ValidatedUuid)
            .map_err(|_| ErrorBadRequest("Invalid UUID format"))
    }

    /// Get the inner UUID value
    pub fn into_inner(self) -> Uuid {
        self.0
    }

    /// Get a reference to the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl From<Uuid> for ValidatedUuid {
    fn from(uuid: Uuid) -> Self {
        ValidatedUuid(uuid)
    }
}

impl std::fmt::Display for ValidatedUuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Validated user ID that ensures authorization
#[derive(Debug, Clone, Copy)]
pub struct AuthorizedUserId {
    user_id: Uuid,
}

impl AuthorizedUserId {
    /// Parse user ID and verify it matches the authenticated user
    /// This eliminates the repeated pattern of parsing + authorization check
    pub fn parse_and_authorize(
        input: &str,
        authenticated_user: &AuthenticatedUser,
    ) -> Result<Self, ActixError> {
        let user_id = ValidatedUuid::parse(input)?.into_inner();

        if user_id != authenticated_user.user_id {
            return Err(ErrorForbidden(
                serde_json::to_string(&ErrorResponse::new(
                    "forbidden",
                    "Cannot access resources for other users",
                ))
                .unwrap_or_default(),
            ));
        }

        Ok(AuthorizedUserId { user_id })
    }

    /// Create from authenticated user (always valid)
    pub fn from_authenticated_user(user: &AuthenticatedUser) -> Self {
        AuthorizedUserId {
            user_id: user.user_id,
        }
    }

    /// Get the user ID
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }
}

/// Validated exchange name that ensures support
#[derive(Debug, Clone)]
pub struct ValidatedExchangeName(String);

impl ValidatedExchangeName {
    /// Supported exchanges — derived from the single source of truth.
    const SUPPORTED_EXCHANGES: &'static [&'static str] = exchanges::SUPPORTED;

    /// Parse and validate exchange name
    pub fn parse(name: &str) -> Result<Self, ActixError> {
        let normalized = name.trim().to_lowercase();

        if normalized.is_empty() {
            return Err(ErrorBadRequest("Exchange name cannot be empty"));
        }

        if !Self::SUPPORTED_EXCHANGES.contains(&normalized.as_str()) {
            return Err(ErrorBadRequest(format!(
                "Exchange '{}' is not supported. Supported exchanges: {}",
                name,
                Self::SUPPORTED_EXCHANGES.join(", ")
            )));
        }

        Ok(ValidatedExchangeName(normalized))
    }

    /// Get the validated exchange name
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to owned string
    pub fn into_string(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_validated_uuid_success() {
        let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
        let validated = ValidatedUuid::parse(uuid_str).unwrap();
        assert_eq!(validated.to_string(), uuid_str);
    }

    #[test]
    fn test_validated_uuid_failure() {
        let result = ValidatedUuid::parse("invalid-uuid");
        assert!(result.is_err());
    }

    #[test]
    fn test_validated_exchange_name_success() {
        let validated = ValidatedExchangeName::parse("  BINANCE  ").unwrap();
        assert_eq!(validated.as_str(), "binance");
    }

    #[test]
    fn test_validated_exchange_name_failure() {
        let result = ValidatedExchangeName::parse("unsupported_exchange");
        assert!(result.is_err());
    }

    #[test]
    fn test_authorized_user_id_success() {
        let user_id = Uuid::new_v4();
        let auth_user = AuthenticatedUser::siwe(
            user_id,
            "0xC285000000000000000000000000000000005b36".to_string(),
        );

        let authorized =
            AuthorizedUserId::parse_and_authorize(&user_id.to_string(), &auth_user).unwrap();
        assert_eq!(authorized.user_id(), user_id);
    }

    #[test]
    fn test_authorized_user_id_forbidden() {
        let user_id = Uuid::new_v4();
        let different_id = Uuid::new_v4();
        let auth_user = AuthenticatedUser::siwe(
            user_id,
            "0xC285000000000000000000000000000000005b36".to_string(),
        );

        let result = AuthorizedUserId::parse_and_authorize(&different_id.to_string(), &auth_user);
        assert!(result.is_err());
    }
}
