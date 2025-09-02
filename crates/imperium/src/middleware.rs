//! middleware (placeholder)

use std::sync::Arc;
// // use crate::auth::JwtAuth;

pub struct AuthMiddleware;
pub struct RateLimitMiddleware;

impl AuthMiddleware {
    pub fn new() -> Self {
        Self
    }
}

impl RateLimitMiddleware {
    pub fn new(_limit: u32) -> Self {
        Self
    }
}

// Example of how it might be used in Axum:
// .layer(tower::ServiceBuilder::new()
//     .layer(AuthMiddleware::new(auth_service))
//     .layer(RateLimitMiddleware::new(100))
// )
