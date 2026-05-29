//! AUTH-02: Authentication and authorization helpers
//!
//! Provides reusable auth patterns for route handlers.

// @anchor exchange:router:auth_helpers
// @tags api

use actix_web::Error as ActixError;

use crate::middleware::AuthenticatedUser;
use crate::utils::validation::{AuthorizedUserId, ValidatedUuid};

/// Authentication context for validated, authorized access patterns
pub struct AuthContext {
    pub user: AuthenticatedUser,
}

impl AuthContext {
    pub fn new(user: AuthenticatedUser) -> Self {
        Self { user }
    }

    /// Get authorized user ID (always valid since it comes from authenticated user)
    pub fn user_id(&self) -> AuthorizedUserId {
        AuthorizedUserId::from_authenticated_user(&self.user)
    }

    /// Parse and authorize a user ID string against the authenticated user
    pub fn authorize_user_id(&self, user_id_str: &str) -> Result<AuthorizedUserId, ActixError> {
        AuthorizedUserId::parse_and_authorize(user_id_str, &self.user)
    }

    /// Parse a resource ID (like order_id, account_id) with validation
    pub fn parse_resource_id(&self, id_str: &str) -> Result<ValidatedUuid, ActixError> {
        ValidatedUuid::parse(id_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_auth_context_user_id() {
        let user_id = Uuid::new_v4();
        let user = AuthenticatedUser {
            user_id,
            wallet_address: "0xC285000000000000000000000000000000005b36".to_string(),
        };

        let ctx = AuthContext::new(user);
        let authorized_id = ctx.user_id();

        assert_eq!(authorized_id.user_id(), user_id);
    }

    #[test]
    fn test_auth_context_authorize_matching_user() {
        let user_id = Uuid::new_v4();
        let user = AuthenticatedUser {
            user_id,
            wallet_address: "0xC285000000000000000000000000000000005b36".to_string(),
        };

        let ctx = AuthContext::new(user);
        let result = ctx.authorize_user_id(&user_id.to_string());

        assert!(result.is_ok());
        assert_eq!(result.unwrap().user_id(), user_id);
    }

    #[test]
    fn test_auth_context_authorize_different_user() {
        let user_id = Uuid::new_v4();
        let different_id = Uuid::new_v4();
        let user = AuthenticatedUser {
            user_id,
            wallet_address: "0xC285000000000000000000000000000000005b36".to_string(),
        };

        let ctx = AuthContext::new(user);
        let result = ctx.authorize_user_id(&different_id.to_string());

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_resource_id_valid() {
        let user_id = Uuid::new_v4();
        let user = AuthenticatedUser {
            user_id,
            wallet_address: "0xC285000000000000000000000000000000005b36".to_string(),
        };

        let ctx = AuthContext::new(user);
        let resource_id = Uuid::new_v4();
        let result = ctx.parse_resource_id(&resource_id.to_string());

        assert!(result.is_ok());
        assert_eq!(result.unwrap().into_inner(), resource_id);
    }

    #[test]
    fn test_parse_resource_id_invalid() {
        let user_id = Uuid::new_v4();
        let user = AuthenticatedUser {
            user_id,
            wallet_address: "0xC285000000000000000000000000000000005b36".to_string(),
        };

        let ctx = AuthContext::new(user);
        let result = ctx.parse_resource_id("invalid-uuid");

        assert!(result.is_err());
    }
}
