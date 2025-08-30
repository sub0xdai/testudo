//! middleware (placeholder)

use std::sync::Arc;
use crate::auth::JwtAuth;

pub struct AuthMiddleware;
pub struct RateLimitMiddleware;

impl AuthMiddleware {
    pub fn new(_auth: Arc<JwtAuth>) -> Self {
        Self
    }
}

impl RateLimitMiddleware {
    pub fn new(_limit: u32) -> Self {
        Self
    }
}