//! Imperium - API Server and Command Interface
//!
//! This crate provides clear command structure and control through REST API endpoints,
//! WebSocket real-time communication, and administrative interfaces. Serves as the
//! primary interface between users and the Testudo trading platform.
//!
//! ## API Architecture
//!
//! - **REST API**: CRUD operations for accounts, trades, and configuration
//! - **WebSocket**: Real-time market data, position updates, and trading events
//! - **GraphQL**: Advanced querying for analytics and reporting (future)
//! - **Admin API**: Platform administration and monitoring interfaces
//!
//! ## Security Model
//!
//! - **JWT Authentication**: Secure token-based user authentication
//! - **Role-Based Access**: Trader, Admin, and API-only access levels
//! - **Rate Limiting**: Per-user and per-endpoint request throttling
//! - **API Key Management**: Secure exchange credential storage
//!
//! ## Roman Military Principle: Imperium
//!
//! Clear command structure and decisive action under pressure. Every API endpoint
//! has clear authority and responsibility, with systematic error handling and logging.

pub mod api;
pub mod websocket;
pub mod auth;
pub mod middleware;
pub mod handlers;
pub mod database;
pub mod cache;
pub mod types;

pub use api::{create_router, ApiState};
pub use websocket::{WebSocketHandler, ConnectionManager};
pub use auth::{
    OidcValidator, SessionManager, AuthMiddleware, 
    UserClaims, AuthContext, AuthService, AuthState
};
pub use handlers::{
    auth_handlers, trade_handlers, account_handlers, 
    market_handlers, admin_handlers
};
pub use types::{
    ApiError, PaginationParams, 
    WebSocketMessage, UserSession
};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use disciplina::{AccountEquity, PositionSize};
use formatio::{TradeIntent, ExecutionResult};
use prudentia::ExchangeAdapterTrait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use thiserror::Error;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};

/// Imperium API server errors
#[derive(Debug, Error)]
pub enum ImperiumError {
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("Authorization failed: required role {required_role}")]
    AuthorizationFailed { required_role: String },
    
    #[error("Database operation failed: {operation}")]
    DatabaseError { operation: String },
    
    #[error("Cache operation failed: {operation}")]
    CacheError { operation: String },
    
    #[error("Invalid request: {field} - {reason}")]
    InvalidRequest { field: String, reason: String },
    
    #[error("Rate limit exceeded: {limit} requests per {window}")]
    RateLimitExceeded { limit: u32, window: String },
    
    #[error("WebSocket connection error: {reason}")]
    WebSocketError { reason: String },
    
    #[error("Exchange operation failed: {source}")]
    ExchangeError { source: prudentia::PrudentiaError },
    
    #[error("Trading operation failed: {source}")]
    TradingError { source: formatio::FormatioError },
    
    #[error("Risk calculation failed: {source}")]
    RiskError { source: disciplina::PositionSizingError },
    
    #[error("Internal server error: {message}")]
    InternalError { message: String },
}

/// Result type for all Imperium operations
pub type Result<T> = std::result::Result<T, ImperiumError>;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    /// Database connection pool
    pub db_pool: PgPool,
    
    /// Redis connection manager
    pub cache: redis::aio::ConnectionManager,
    
    /// Van Tharp position size calculator
    pub risk_calculator: Arc<disciplina::PositionSizingCalculator>,
    
    /// OODA loop trading controller
    pub trading_controller: Arc<formatio::OodaController>,
    
    /// Exchange adapter manager
    pub exchange_manager: Arc<prudentia::FailoverManager>,
    
    /// WebSocket connection manager
    pub websocket_manager: Arc<WebSocketHandler>,
    
    /// Application configuration
    pub config: AppConfig,
    
    /// Authentication service with OIDC validator
    pub auth_service: Arc<AuthService>,
}

/// Configuration for the Imperium API server
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Server listening address
    pub server_host: String,
    pub server_port: u16,
    
    /// Database configuration
    pub database_url: String,
    pub database_pool_size: u32,
    
    /// Redis configuration
    pub redis_url: String,
    
    /// JWT configuration
    pub jwt_secret: String,
    pub jwt_expiration_hours: u32,
    
    /// CORS settings
    pub cors_allowed_origins: Vec<String>,
    
    /// Rate limiting
    pub rate_limit_requests_per_minute: u32,
    
    /// WebSocket settings
    pub websocket_max_connections: u32,
    pub websocket_heartbeat_interval: u32,
}

/// Standard API response wrapper
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }
    
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Implement IntoResponse for ImperiumError
impl IntoResponse for ImperiumError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ImperiumError::AuthenticationFailed { .. } => {
                (StatusCode::UNAUTHORIZED, self.to_string())
            },
            ImperiumError::AuthorizationFailed { .. } => {
                (StatusCode::FORBIDDEN, self.to_string())
            },
            ImperiumError::InvalidRequest { .. } => {
                (StatusCode::BAD_REQUEST, self.to_string())
            },
            ImperiumError::RateLimitExceeded { .. } => {
                (StatusCode::TOO_MANY_REQUESTS, self.to_string())
            },
            _ => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            },
        };
        
        let response = ApiResponse::<()>::error(message);
        (status, Json(response)).into_response()
    }
}

/// Create the main application router with all middleware and routes
pub fn create_app_router(state: AppState) -> axum::Router {
    axum::Router::new()
        .nest("/api/v1", api::create_router())
        .nest("/ws", websocket::create_router())
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()) // Configure properly for production
                // TODO: Add middleware when implementations are complete
                // .layer(middleware::AuthMiddleware::new(state.auth_service.clone()))
                // .layer(middleware::RateLimitMiddleware::new(
                //     state.config.rate_limit_requests_per_minute
                // ))
        )
        .with_state(state)
}

/// Health check endpoint for load balancer and monitoring
pub async fn health_check(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>> {
    // Check database connectivity
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.db_pool).await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };
    
    // Check Redis connectivity  
    let mut cache = state.cache.clone();
    let cache_status = match redis::cmd("PING").query_async::<_, String>(&mut cache).await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };
    
    let health_data = serde_json::json!({
        "service": "testudo-imperium",
        "version": env!("CARGO_PKG_VERSION"),
        "database": db_status,
        "cache": cache_status,
        "timestamp": chrono::Utc::now()
    });
    
    Ok(Json(ApiResponse::success(health_data)))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }
    
    #[test]
    fn test_api_response_error() {
        let response = ApiResponse::<()>::error("test error".to_string());
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
    }
}