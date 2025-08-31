//! websocket (placeholder)

use axum::Router;
use crate::AppState;

pub struct WebSocketHandler;
pub struct ConnectionManager;

pub fn create_router() -> Router<AppState> {
    Router::new()
}