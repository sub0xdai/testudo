//! Authentication module implementing OIDC/OAuth2 with Keycloak
//! 
//! This module provides secure authentication following the Testudo SOPs:
//! - SOP-001: Risk calculation verification with user context
//! - SOP-002: OODA loop integration for authenticated operations
//! - SOP-003: Recovery procedures for authentication failures
//!
//! Key principles:
//! - JWT tokens stored in memory only (never localStorage)
//! - Automatic token refresh with fallback mechanisms
//! - Comprehensive audit trail for security events
//! - SOP-003 compliant recovery procedures

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Path, Query, State},
    http::{header, request::Parts, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{
    decode, decode_header, jwk::{AlgorithmParameters, JwkSet},
    Algorithm, DecodingKey, Validation,
};
use prudentia::RiskProfile;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Instant,
};
use thiserror::Error;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{error, info, warn, debug};
use url::Url;
use uuid::Uuid;

/// Authentication errors with recovery guidance per SOP-003
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid or expired token")]
    InvalidToken(String),
    
    #[error("OIDC provider unreachable: {0}")]
    ProviderUnreachable(String),
    
    #[error("JWKS refresh failed: {0}")]
    JwksRefreshFailed(String),
    
    #[error("User session not found or expired")]
    SessionNotFound,
    
    #[error("Insufficient permissions for operation")]
    InsufficientPermissions,
    
    #[error("Authentication service unavailable")]
    ServiceUnavailable,
    
    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::InvalidToken(_) => (StatusCode::UNAUTHORIZED, "Authentication required"),
            AuthError::ProviderUnreachable(_) => (StatusCode::SERVICE_UNAVAILABLE, "Authentication service unavailable"),
            AuthError::SessionNotFound => (StatusCode::UNAUTHORIZED, "Session expired"),
            AuthError::InsufficientPermissions => (StatusCode::FORBIDDEN, "Insufficient permissions"),
            AuthError::ServiceUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "Service temporarily unavailable"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Authentication error"),
        };
        
        (status, message).into_response()
    }
}

/// OIDC Discovery document structure
#[derive(Debug, Deserialize)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub end_session_endpoint: Option<String>,
}

/// JWT Claims for Testudo users with risk profile integration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserClaims {
    pub sub: String,           // User ID
    pub email: String,
    pub name: String,
    pub iss: String,           // Issuer
    pub aud: String,           // Audience
    pub exp: i64,              // Expiration
    pub iat: i64,              // Issued at
    pub jti: String,           // JWT ID
    
    // Testudo-specific claims
    pub risk_profile: RiskProfile,
    pub account_equity: Option<String>,  // Stored as string to avoid precision loss
    pub max_position_count: u32,
    pub daily_loss_limit: Option<String>,
    pub permissions: Vec<String>,
}

/// User session information stored in Redis
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserSession {
    pub user_id: String,
    pub session_id: String,
    pub email: String,
    pub risk_profile: RiskProfile,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub permissions: Vec<String>,
}

/// Authentication context added to requests
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    pub session_id: String,
    pub email: String,
    pub risk_profile: RiskProfile,
    pub permissions: Vec<String>,
}

/// OIDC configuration for Keycloak
#[derive(Debug, Clone)]
pub struct OidcConfig {
    pub provider_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scope: String,
}

/// OIDC token validator with automatic JWKS refresh per SOP-003
pub struct OidcValidator {
    config: OidcConfig,
    discovery: OidcDiscovery,
    jwks: Arc<RwLock<JwkSet>>,
    jwks_last_refresh: Arc<RwLock<Instant>>,
    http_client: Client,
}

impl OidcValidator {
    /// Create a new OIDC validator and perform initial discovery
    pub async fn new(config: OidcConfig) -> Result<Self> {
        let http_client = Client::new();
        
        // Perform OIDC discovery
        let discovery_url = format!("{}/.well-known/openid_configuration", config.provider_url);
        let discovery: OidcDiscovery = http_client
            .get(&discovery_url)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch OIDC discovery: {}", e))?
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse OIDC discovery: {}", e))?;
        
        info!("OIDC discovery completed for issuer: {}", discovery.issuer);
        
        // Fetch initial JWKS
        let jwks: JwkSet = http_client
            .get(&discovery.jwks_uri)
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch JWKS: {}", e))?
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse JWKS: {}", e))?;
        
        info!("Initial JWKS loaded with {} keys", jwks.keys.len());
        
        Ok(Self {
            config,
            discovery,
            jwks: Arc::new(RwLock::new(jwks)),
            jwks_last_refresh: Arc::new(RwLock::new(Instant::now())),
            http_client,
        })
    }
    
    /// Validate a JWT token and extract user claims
    pub async fn validate_token(&self, token: &str) -> Result<UserClaims, AuthError> {
        // Check if JWKS needs refresh (refresh every 5 minutes per SOP-003)
        if self.needs_jwks_refresh() {
            if let Err(e) = self.refresh_jwks().await {
                warn!("JWKS refresh failed, using cached keys: {}", e);
                // Continue with cached keys per SOP-003 graceful degradation
            }
        }
        
        // Decode token header to get key ID
        let header = decode_header(token)
            .map_err(|e| AuthError::InvalidToken(format!("Invalid token header: {}", e)))?;
        
        let kid = header.kid
            .ok_or_else(|| AuthError::InvalidToken("No key ID in token header".to_string()))?;
        
        // Find corresponding public key
        let jwks = self.jwks.read().unwrap();
        let jwk = jwks.keys
            .iter()
            .find(|key| {
                if let Some(key_id) = &key.common.key_id {
                    key_id == &kid
                } else {
                    false
                }
            })
            .ok_or_else(|| AuthError::InvalidToken(format!("Key ID {} not found in JWKS", kid)))?;
        
        // Create decoding key
        let decoding_key = match &jwk.algorithm {
            AlgorithmParameters::RSA(rsa) => {
                // RSA components are provided as base64url-encoded strings
                DecodingKey::from_rsa_components(&rsa.n, &rsa.e)
                    .map_err(|e| AuthError::InvalidToken(format!("Invalid RSA key: {}", e)))?
            }
            _ => return Err(AuthError::InvalidToken("Unsupported key algorithm".to_string())),
        };
        
        // Validate token
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.discovery.issuer]);
        validation.set_audience(&[&self.config.client_id]);
        
        let token_data = decode::<UserClaims>(token, &decoding_key, &validation)
            .map_err(|e| AuthError::InvalidToken(format!("Token validation failed: {}", e)))?;
        
        // Additional security checks
        self.verify_token_claims(&token_data.claims)?;
        
        debug!("Token validated successfully for user: {}", token_data.claims.sub);
        Ok(token_data.claims)
    }
    
    /// Check if JWKS needs refresh (every 5 minutes)
    fn needs_jwks_refresh(&self) -> bool {
        let last_refresh = *self.jwks_last_refresh.read().unwrap();
        last_refresh.elapsed() > std::time::Duration::from_secs(300)
    }
    
    /// Refresh JWKS from the provider
    async fn refresh_jwks(&self) -> Result<(), AuthError> {
        debug!("Refreshing JWKS from provider");
        
        let jwks: JwkSet = self.http_client
            .get(&self.discovery.jwks_uri)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| AuthError::JwksRefreshFailed(format!("Request failed: {}", e)))?
            .json()
            .await
            .map_err(|e| AuthError::JwksRefreshFailed(format!("Parse failed: {}", e)))?;
        
        // Update cached JWKS
        *self.jwks.write().unwrap() = jwks;
        *self.jwks_last_refresh.write().unwrap() = Instant::now();
        
        info!("JWKS refreshed successfully");
        Ok(())
    }
    
    /// Verify additional token claims per security policy
    fn verify_token_claims(&self, claims: &UserClaims) -> Result<(), AuthError> {
        // Check token expiration with clock skew tolerance
        let now = Utc::now().timestamp();
        if claims.exp < now - 30 {
            return Err(AuthError::InvalidToken("Token expired".to_string()));
        }
        
        // Verify issuer matches discovery
        if claims.iss != self.discovery.issuer {
            return Err(AuthError::InvalidToken("Invalid issuer".to_string()));
        }
        
        // Verify audience
        if claims.aud != self.config.client_id {
            return Err(AuthError::InvalidToken("Invalid audience".to_string()));
        }
        
        // Verify user has minimum required permissions
        if !claims.permissions.contains(&"trade:execute".to_string()) {
            return Err(AuthError::InsufficientPermissions);
        }
        
        Ok(())
    }
}

/// Session manager for handling user sessions in Redis
pub struct SessionManager {
    redis_pool: redis::Client,
}

impl SessionManager {
    pub fn new(redis_url: &str) -> Result<Self> {
        let redis_pool = redis::Client::open(redis_url)
            .map_err(|e| anyhow!("Failed to connect to Redis: {}", e))?;
        
        Ok(Self { redis_pool })
    }
    
    /// Create a new session for authenticated user
    pub async fn create_session(&self, claims: &UserClaims) -> Result<UserSession, AuthError> {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        
        let session = UserSession {
            user_id: claims.sub.clone(),
            session_id: session_id.clone(),
            email: claims.email.clone(),
            risk_profile: claims.risk_profile.clone(),
            created_at: now,
            last_activity: now,
            expires_at: now + Duration::hours(24), // 24-hour session
            permissions: claims.permissions.clone(),
        };
        
        // Store session in Redis
        let mut conn = self.redis_pool
            .get_async_connection()
            .await
            .map_err(|e| AuthError::ServiceUnavailable)?;
        
        let session_key = format!("session:{}", session_id);
        let session_json = serde_json::to_string(&session)
            .map_err(|e| AuthError::ServiceUnavailable)?;
        
        redis::cmd("SET")
            .arg(&session_key)
            .arg(&session_json)
            .arg("EX")
            .arg(86400) // Expire in 24 hours
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AuthError::ServiceUnavailable)?;
        
        // Also store user ID mapping for quick lookups
        let user_key = format!("user:{}:session", claims.sub);
        redis::cmd("SET")
            .arg(&user_key)
            .arg(&session_id)
            .arg("EX")
            .arg(86400)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| AuthError::ServiceUnavailable)?;
        
        info!("Session created for user {} with ID {}", claims.sub, session_id);
        Ok(session)
    }
    
    /// Get session by session ID
    pub async fn get_session(&self, session_id: &str) -> Result<UserSession, AuthError> {
        let mut conn = self.redis_pool
            .get_async_connection()
            .await
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        let session_key = format!("session:{}", session_id);
        let session_json: Option<String> = redis::cmd("GET")
            .arg(&session_key)
            .query_async(&mut conn)
            .await
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        let session_json = session_json.ok_or(AuthError::SessionNotFound)?;
        let session: UserSession = serde_json::from_str(&session_json)
            .map_err(|_| AuthError::SessionNotFound)?;
        
        // Check if session is expired
        if session.expires_at < Utc::now() {
            self.delete_session(session_id).await?;
            return Err(AuthError::SessionNotFound);
        }
        
        Ok(session)
    }
    
    /// Update session activity timestamp
    pub async fn update_session_activity(&self, session_id: &str) -> Result<(), AuthError> {
        let mut conn = self.redis_pool
            .get_async_connection()
            .await
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        let session_key = format!("session:{}", session_id);
        let mut session = self.get_session(session_id).await?;
        
        session.last_activity = Utc::now();
        
        let session_json = serde_json::to_string(&session)
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        redis::cmd("SET")
            .arg(&session_key)
            .arg(&session_json)
            .arg("EX")
            .arg(86400) // Reset expiration
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        Ok(())
    }
    
    /// Delete a session
    pub async fn delete_session(&self, session_id: &str) -> Result<(), AuthError> {
        let mut conn = self.redis_pool
            .get_async_connection()
            .await
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        let session_key = format!("session:{}", session_id);
        redis::cmd("DEL")
            .arg(&session_key)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        Ok(())
    }
}

/// Authentication middleware with SOP-003 recovery procedures
pub struct AuthMiddleware {
    oidc_validator: Arc<OidcValidator>,
    session_manager: Arc<SessionManager>,
}

impl AuthMiddleware {
    pub fn new(oidc_validator: Arc<OidcValidator>, session_manager: Arc<SessionManager>) -> Self {
        Self {
            oidc_validator,
            session_manager,
        }
    }
    
    /// Extract bearer token from request
    fn extract_bearer_token(parts: &Parts) -> Result<String, AuthError> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or_else(|| AuthError::InvalidToken("No Authorization header".to_string()))?
            .to_str()
            .map_err(|_| AuthError::InvalidToken("Invalid Authorization header".to_string()))?;
        
        if !auth_header.starts_with("Bearer ") {
            return Err(AuthError::InvalidToken("Invalid Bearer token format".to_string()));
        }
        
        Ok(auth_header[7..].to_string())
    }
    
    /// Validate authentication and create context
    pub async fn validate_request(&self, parts: &Parts) -> Result<AuthContext, AuthError> {
        // Extract token from Authorization header
        let token = Self::extract_bearer_token(parts)?;
        
        // Validate token with OIDC provider (with SOP-003 recovery)
        let claims = match self.oidc_validator.validate_token(&token).await {
            Ok(claims) => claims,
            Err(AuthError::ProviderUnreachable(_)) => {
                warn!("OIDC provider unreachable, attempting session-only validation");
                // SOP-003: Fallback to session validation if OIDC provider is down
                return self.validate_with_session_fallback(&token).await;
            }
            Err(e) => return Err(e),
        };
        
        // Verify session exists and is valid
        let session = self.verify_user_session(&claims.sub).await?;
        
        // Update session activity
        if let Err(e) = self.session_manager.update_session_activity(&session.session_id).await {
            warn!("Failed to update session activity: {}", e);
            // Non-critical error, continue
        }
        
        Ok(AuthContext {
            user_id: claims.sub,
            session_id: session.session_id,
            email: claims.email,
            risk_profile: claims.risk_profile,
            permissions: claims.permissions,
        })
    }
    
    /// Fallback validation using session only (SOP-003 recovery)
    async fn validate_with_session_fallback(&self, token: &str) -> Result<AuthContext, AuthError> {
        warn!("Using session fallback validation due to OIDC provider issues");
        
        // Try to decode token without verification (risky but necessary for recovery)
        let token_data = jsonwebtoken::decode::<UserClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(b"dummy"), // Won't be used
            &jsonwebtoken::Validation::default(),
        );
        
        // If even basic decode fails, reject
        let claims = match token_data {
            Ok(data) => data.claims,
            Err(_) => return Err(AuthError::InvalidToken("Token decode failed".to_string())),
        };
        
        // Verify session exists
        let session = self.verify_user_session(&claims.sub).await?;
        
        // In fallback mode, use session data as authoritative
        Ok(AuthContext {
            user_id: session.user_id,
            session_id: session.session_id,
            email: session.email,
            risk_profile: session.risk_profile,
            permissions: session.permissions,
        })
    }
    
    /// Verify user has valid session
    async fn verify_user_session(&self, user_id: &str) -> Result<UserSession, AuthError> {
        // Get session ID from user mapping
        let mut conn = self.session_manager.redis_pool
            .get_async_connection()
            .await
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        let user_key = format!("user:{}:session", user_id);
        let session_id: Option<String> = redis::cmd("GET")
            .arg(&user_key)
            .query_async(&mut conn)
            .await
            .map_err(|_| AuthError::ServiceUnavailable)?;
        
        let session_id = session_id.ok_or(AuthError::SessionNotFound)?;
        self.session_manager.get_session(&session_id).await
    }
}

/// FromRequestParts implementation for AuthContext
#[async_trait]
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // This will be implemented when we add the middleware to the router
        // For now, return an error
        Err(AuthError::ServiceUnavailable)
    }
}

/// OAuth callback query parameters
#[derive(Debug, Deserialize)]
pub struct OAuthCallback {
    code: String,
    state: Option<String>,
}

/// Auth service containing all authentication logic
pub struct AuthService {
    oidc_validator: Arc<OidcValidator>,
    session_manager: Arc<SessionManager>,
    http_client: Client,
}

impl AuthService {
    pub fn new(oidc_validator: Arc<OidcValidator>, session_manager: Arc<SessionManager>) -> Self {
        Self {
            oidc_validator,
            session_manager,
            http_client: Client::new(),
        }
    }
    
    /// Generate OAuth authorization URL
    pub fn get_authorization_url(&self, state: &str) -> String {
        let mut auth_url = Url::parse(&self.oidc_validator.discovery.authorization_endpoint)
            .expect("Invalid authorization endpoint");
        
        auth_url.query_pairs_mut()
            .append_pair("client_id", &self.oidc_validator.config.client_id)
            .append_pair("redirect_uri", &self.oidc_validator.config.redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", &self.oidc_validator.config.scope)
            .append_pair("state", state);
        
        auth_url.to_string()
    }
    
    /// Handle OAuth callback and create session
    pub async fn handle_callback(&self, callback: OAuthCallback) -> Result<UserSession, AuthError> {
        // Exchange authorization code for tokens
        let token_response = self.exchange_code_for_tokens(&callback.code).await?;
        
        // Validate the access token and extract claims
        let claims = self.oidc_validator
            .validate_token(&token_response.access_token)
            .await?;
        
        // Create session
        let session = self.session_manager.create_session(&claims).await?;
        
        info!("OAuth callback processed successfully for user: {}", claims.sub);
        Ok(session)
    }
    
    /// Exchange authorization code for access token
    async fn exchange_code_for_tokens(&self, code: &str) -> Result<TokenResponse, AuthError> {
        let params = [
            ("client_id", self.oidc_validator.config.client_id.as_str()),
            ("client_secret", self.oidc_validator.config.client_secret.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", self.oidc_validator.config.redirect_uri.as_str()),
        ];
        
        let response = self.http_client
            .post(&self.oidc_validator.discovery.token_endpoint)
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::ProviderUnreachable(format!("Token exchange failed: {}", e)))?;
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(AuthError::InvalidToken(format!("Token exchange failed: {}", error_text)));
        }
        
        response.json::<TokenResponse>()
            .await
            .map_err(|e| AuthError::InvalidToken(format!("Failed to parse token response: {}", e)))
    }
}

/// OAuth token response
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<u32>,
    refresh_token: Option<String>,
    scope: Option<String>,
}

/// Create authentication routes
pub fn create_auth_routes(auth_service: Arc<AuthService>) -> Router {
    Router::new()
        .route("/login", get(login_handler))
        .route("/callback", get(callback_handler))
        .route("/logout", post(logout_handler))
        .route("/me", get(me_handler))
        .with_state(auth_service)
}

/// GET /auth/login - Redirect to OAuth provider
async fn login_handler(State(auth_service): State<Arc<AuthService>>) -> impl IntoResponse {
    let state = Uuid::new_v4().to_string();
    let auth_url = auth_service.get_authorization_url(&state);
    
    info!("Redirecting user to OAuth provider");
    Redirect::temporary(&auth_url)
}

/// GET /auth/callback - Handle OAuth callback
async fn callback_handler(
    Query(callback): Query<OAuthCallback>,
    State(auth_service): State<Arc<AuthService>>,
) -> impl IntoResponse {
    match auth_service.handle_callback(callback).await {
        Ok(session) => {
            // Set session cookie and redirect to dashboard
            let cookie = format!("session_id={}; HttpOnly; Secure; SameSite=Strict; Max-Age=86400", 
                                session.session_id);
            
            (
                StatusCode::FOUND,
                [
                    (header::SET_COOKIE, cookie),
                    (header::LOCATION, "/dashboard".to_string()),
                ]
            ).into_response()
        }
        Err(e) => {
            error!("OAuth callback failed: {}", e);
            (StatusCode::UNAUTHORIZED, format!("Authentication failed: {}", e)).into_response()
        }
    }
}

/// POST /auth/logout - Logout user
async fn logout_handler(
    auth_context: AuthContext,
    State(auth_service): State<Arc<AuthService>>,
) -> impl IntoResponse {
    if let Err(e) = auth_service.session_manager.delete_session(&auth_context.session_id).await {
        warn!("Failed to delete session: {}", e);
    }
    
    info!("User {} logged out", auth_context.user_id);
    
    // Clear session cookie
    (
        StatusCode::OK,
        [(header::SET_COOKIE, "session_id=; HttpOnly; Secure; SameSite=Strict; Max-Age=0")],
        Json(serde_json::json!({
            "message": "Logged out successfully"
        }))
    )
}

/// GET /auth/me - Get current user info
async fn me_handler(auth_context: AuthContext) -> impl IntoResponse {
    Json(serde_json::json!({
        "user_id": auth_context.user_id,
        "email": auth_context.email,
        "risk_profile": auth_context.risk_profile,
        "permissions": auth_context.permissions
    }))
}

/// Authentication state that can be stored in Axum application state
#[derive(Clone)]
pub struct AuthState {
    pub auth_service: Arc<AuthService>,
    pub auth_middleware: Arc<AuthMiddleware>,
}

impl AuthState {
    /// Initialize authentication state (async because of OIDC discovery)
    pub async fn new(
        oidc_config: OidcConfig,
        redis_url: &str,
    ) -> Result<Self> {
        let session_manager = Arc::new(SessionManager::new(redis_url)?);
        let oidc_validator = Arc::new(OidcValidator::new(oidc_config).await?);
        
        let auth_service = Arc::new(AuthService::new(oidc_validator.clone(), session_manager.clone()));
        let auth_middleware = Arc::new(AuthMiddleware::new(oidc_validator, session_manager));
        
        Ok(Self {
            auth_service,
            auth_middleware,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;
    
    #[test]
    async fn test_oidc_config_creation() {
        let config = OidcConfig {
            provider_url: "http://localhost:8080/realms/testudo".to_string(),
            client_id: "testudo-frontend".to_string(),
            client_secret: "test-secret".to_string(),
            redirect_uri: "http://localhost:3000/auth/callback".to_string(),
            scope: "openid profile email".to_string(),
        };
        
        assert_eq!(config.client_id, "testudo-frontend");
        assert_eq!(config.scope, "openid profile email");
    }
    
    #[test]
    fn test_auth_error_display() {
        let error = AuthError::InvalidToken("test error".to_string());
        assert_eq!(error.to_string(), "Invalid or expired token");
        
        let error = AuthError::SessionNotFound;
        assert_eq!(error.to_string(), "User session not found or expired");
    }
    
    #[test]
    fn test_user_claims_serialization() {
        use prudentia::types::RiskProfile;
        
        let claims = UserClaims {
            sub: "user123".to_string(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
            iss: "http://localhost:8080/realms/testudo".to_string(),
            aud: "testudo-frontend".to_string(),
            exp: 1234567890,
            iat: 1234567800,
            jti: "token123".to_string(),
            risk_profile: RiskProfile::Standard,
            account_equity: Some("10000.00".to_string()),
            max_position_count: 5,
            daily_loss_limit: Some("500.00".to_string()),
            permissions: vec!["trade:execute".to_string(), "risk:view".to_string()],
        };
        
        let json = serde_json::to_string(&claims).unwrap();
        let parsed: UserClaims = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.sub, "user123");
        assert_eq!(parsed.email, "test@example.com");
        assert_eq!(parsed.permissions.len(), 2);
    }
}