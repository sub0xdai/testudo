//! API routes (placeholder)

use axum::Router;
use crate::AppState;

pub struct ApiState;

pub fn create_router() -> Router<AppState> {
    Router::new()
}