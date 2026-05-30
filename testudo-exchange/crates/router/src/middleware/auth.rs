// @anchor exchange:router:auth
// @tags api

use actix_web::{
    body::BoxBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    error::ErrorUnauthorized,
    http::header::{HeaderName, HeaderValue},
    web, Error, FromRequest, HttpMessage, HttpRequest, HttpResponse,
};
use common_utils::auth::{TokenClaims, TokenService};
use futures_util::future::{ready, Ready};

use crate::models::agent_key::{AgentKeyClaims, AgentPermission, AuthMethod};
use crate::services::agent_key;
use std::{
    collections::HashMap,
    future::Future,
    net::IpAddr,
    pin::Pin,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// Rate limiter for preventing brute force attacks
#[derive(Debug)]
pub struct RateLimiter {
    /// Track attempts per IP address
    attempts: Mutex<HashMap<IpAddr, Vec<Instant>>>,
    /// Maximum attempts allowed
    max_attempts: usize,
    /// Time window for tracking attempts
    time_window: Duration,
}

impl RateLimiter {
    pub fn new(max_attempts: usize, time_window: Duration) -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
            max_attempts,
            time_window,
        }
    }

    /// Check if IP is allowed to make a request
    pub fn is_allowed(&self, ip: IpAddr) -> bool {
        let mut attempts = match self.attempts.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("Rate limiter lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        let now = Instant::now();

        // Clean up old attempts
        let cutoff = now - self.time_window;
        attempts
            .entry(ip)
            .or_insert_with(Vec::new)
            .retain(|&attempt| attempt > cutoff);

        attempts
            .get(&ip)
            .map_or(true, |ip_attempts| ip_attempts.len() < self.max_attempts)
    }

    /// Record an attempt for an IP
    pub fn record_attempt(&self, ip: IpAddr) {
        let mut attempts = match self.attempts.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                tracing::warn!("Rate limiter lock was poisoned, recovering");
                poisoned.into_inner()
            }
        };
        attempts
            .entry(ip)
            .or_insert_with(Vec::new)
            .push(Instant::now());
    }
}

/// JWT middleware for validating authentication tokens (AUTH-02: dual extraction)
pub struct JwtMiddleware {
    token_service: Arc<dyn TokenService>,
    rate_limiter: Arc<RateLimiter>,
}

impl JwtMiddleware {
    pub fn new(token_service: Arc<dyn TokenService>) -> Self {
        Self {
            token_service,
            // Default: 10 attempts per 15 minutes
            rate_limiter: Arc::new(RateLimiter::new(10, Duration::from_secs(900))),
        }
    }

    pub fn with_rate_limit(
        token_service: Arc<dyn TokenService>,
        max_attempts: usize,
        time_window: Duration,
    ) -> Self {
        Self {
            token_service,
            rate_limiter: Arc::new(RateLimiter::new(max_attempts, time_window)),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for JwtMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: actix_web::body::MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = JwtMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(JwtMiddlewareService {
            service: Rc::new(service),
            token_service: self.token_service.clone(),
            rate_limiter: self.rate_limiter.clone(),
        }))
    }
}

pub struct JwtMiddlewareService<S> {
    service: Rc<S>,
    token_service: Arc<dyn TokenService>,
    rate_limiter: Arc<RateLimiter>,
}

/// Helper to create error response.
/// Does NOT set CORS headers — the outer Actix CORS middleware handles them.
/// Setting `Access-Control-Allow-Origin: *` here would conflict with
/// `supports_credentials()` and cause browsers to reject the response.
fn error_response(
    req: ServiceRequest,
    status: actix_web::http::StatusCode,
    message: &str,
) -> ServiceResponse<BoxBody> {
    let response = HttpResponse::build(status)
        .json(serde_json::json!({"error": message}));
    req.into_response(response)
}

/// AUTH-02 FR-14: Extract token from Bearer header OR access_token cookie.
/// Bearer takes priority over cookie.
fn extract_token(req: &ServiceRequest) -> Option<String> {
    // Priority 1: Authorization: Bearer header (extension)
    if let Some(auth_header) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    // Priority 2: HttpOnly cookie (web/journal)
    req.cookie("access_token").map(|c| c.value().to_string())
}

impl<S, B> Service<ServiceRequest> for JwtMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: actix_web::body::MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let token_service = self.token_service.clone();
        let rate_limiter = self.rate_limiter.clone();

        Box::pin(async move {
            // Extract client IP for rate limiting
            let client_ip = req
                .connection_info()
                .peer_addr()
                .and_then(|addr| addr.parse::<std::net::SocketAddr>().ok())
                .map(|addr| addr.ip())
                .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));

            // Check rate limit
            if !rate_limiter.is_allowed(client_ip) {
                rate_limiter.record_attempt(client_ip);
                return Ok(error_response(
                    req,
                    actix_web::http::StatusCode::TOO_MANY_REQUESTS,
                    "Rate limit exceeded",
                ));
            }

            // AUTH-02 FR-14: Dual token extraction (Bearer + cookie)
            if let Some(token) = extract_token(&req) {
                match token_service.verify_access_token(&token) {
                    Ok(claims) => {
                        // Insert claims into request extensions
                        req.extensions_mut().insert(claims);

                        // Call the service and add security headers
                        let res = service.call(req).await?;
                        let mut res = res.map_body(|_, body| BoxBody::new(body));
                        let headers = res.headers_mut();

                        headers.insert(
                            HeaderName::from_static("x-frame-options"),
                            HeaderValue::from_static("DENY"),
                        );
                        headers.insert(
                            HeaderName::from_static("content-security-policy"),
                            HeaderValue::from_static(
                                "default-src 'self'; script-src 'self'; object-src 'none'",
                            ),
                        );
                        headers.insert(
                            HeaderName::from_static("x-content-type-options"),
                            HeaderValue::from_static("nosniff"),
                        );
                        headers.insert(
                            HeaderName::from_static("x-xss-protection"),
                            HeaderValue::from_static("1; mode=block"),
                        );
                        headers.insert(
                            HeaderName::from_static("referrer-policy"),
                            HeaderValue::from_static("strict-origin-when-cross-origin"),
                        );
                        headers.insert(
                            HeaderName::from_static("strict-transport-security"),
                            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
                        );

                        return Ok(res);
                    }
                    Err(_) => {
                        // Invalid token provided — this IS a brute-force signal
                        rate_limiter.record_attempt(client_ip);
                        return Ok(error_response(
                            req,
                            actix_web::http::StatusCode::UNAUTHORIZED,
                            "Invalid token",
                        ));
                    }
                }
            }

            // No Bearer token — try X-Agent-Key header (AGENT-07)
            if let Some(agent_key_header) = req.headers().get("x-agent-key") {
                if let Ok(key_str) = agent_key_header.to_str() {
                    if let Some(state) = req.app_data::<web::Data<crate::types::app::AppState>>()
                    {
                        let pool = state.pool.clone();
                        match agent_key::resolve_agent_key(&pool, key_str).await {
                            Ok(Some(claims)) => {
                                req.extensions_mut().insert(claims);
                                let res = service.call(req).await?;
                                return Ok(res.map_body(|_, body| BoxBody::new(body)));
                            }
                            Ok(None) => {
                                return Ok(error_response(
                                    req,
                                    actix_web::http::StatusCode::UNAUTHORIZED,
                                    "Invalid or expired agent key",
                                ));
                            }
                            Err(e) => {
                                tracing::error!("Agent key resolution error: {}", e);
                                return Ok(error_response(
                                    req,
                                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                                    "Authentication service error",
                                ));
                            }
                        }
                    }
                }
            }

            // No token provided — legitimate "not logged in" state.
            // Do NOT rate-limit: /auth/me probes on page load are expected
            // when no session exists. Only invalid tokens count as attempts.
            Ok(error_response(
                req,
                actix_web::http::StatusCode::UNAUTHORIZED,
                "Missing or invalid authorization header",
            ))
        })
    }
}

/// Request extractor for authenticated users (AUTH-02: wallet_address replaces email)
/// Supports both SIWE bearer tokens and AGENT-07 scoped agent keys.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: uuid::Uuid,
    pub wallet_address: String,
    /// How this request was authenticated. SIWE = full access, AgentKey = scoped.
    pub auth_method: AuthMethod,
}

impl AuthenticatedUser {
    /// Convenience constructor for SIWE-authenticated users (used in tests).
    pub fn siwe(user_id: uuid::Uuid, wallet_address: String) -> Self {
        Self {
            user_id,
            wallet_address,
            auth_method: AuthMethod::Siwe,
        }
    }

    /// Check if this authenticated principal has a specific permission.
    /// SIWE-authenticated users have all permissions (full access).
    pub fn has_permission(&self, perm: &AgentPermission) -> bool {
        match &self.auth_method {
            AuthMethod::Siwe => true,
            AuthMethod::AgentKey { permissions, .. } => permissions.contains(perm),
        }
    }

    /// Assert that the user has a permission, returning 403 Forbidden if not.
    /// SIWE-authenticated users always pass.
    pub fn require_permission(&self, perm: &AgentPermission) -> Result<(), actix_web::Error> {
        if self.has_permission(perm) {
            Ok(())
        } else {
            Err(actix_web::error::ErrorForbidden(format!(
                "Agent key lacks permission: {:?}. Required: {:?}",
                match &self.auth_method {
                    AuthMethod::AgentKey { permissions, .. } =>
                        format!("{:?}", permissions),
                    AuthMethod::Siwe => "full".into(),
                },
                perm,
            )))
        }
    }

    /// The agent key ID if authenticated via agent key, None for SIWE.
    pub fn agent_key_id(&self) -> Option<uuid::Uuid> {
        match &self.auth_method {
            AuthMethod::AgentKey { key_id, .. } => Some(*key_id),
            AuthMethod::Siwe => None,
        }
    }
}

impl FromRequest for AuthenticatedUser {
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut actix_web::dev::Payload) -> Self::Future {
        // AGENT-07: Check for agent key claims first
        if let Some(claims) = req.extensions().get::<AgentKeyClaims>() {
            return ready(Ok(AuthenticatedUser {
                user_id: claims.user_id,
                wallet_address: String::new(),
                auth_method: AuthMethod::AgentKey {
                    key_id: claims.key_id,
                    permissions: claims.permissions.clone(),
                },
            }));
        }

        if let Some(claims) = req.extensions().get::<TokenClaims>() {
            match uuid::Uuid::parse_str(&claims.sub) {
                Ok(user_id) => ready(Ok(AuthenticatedUser {
                    user_id,
                    wallet_address: claims.wallet_address.clone(),
                    auth_method: AuthMethod::Siwe,
                })),
                Err(_) => ready(Err(ErrorUnauthorized("Invalid user ID in token"))),
            }
        } else {
            ready(Err(ErrorUnauthorized("User not authenticated")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};
    use chrono::Utc;
    use common_utils::auth::{AuthError, TokenClaims, TokenType};
    use std::sync::Arc;
    use uuid::Uuid;

    async fn test_handler(user: AuthenticatedUser) -> HttpResponse {
        HttpResponse::Ok().json(serde_json::json!({
            "user_id": user.user_id,
            "wallet_address": user.wallet_address
        }))
    }

    // Mock TokenService for testing
    struct MockTokenService {
        valid_tokens: std::collections::HashMap<String, TokenClaims>,
    }

    impl MockTokenService {
        fn new() -> Self {
            Self {
                valid_tokens: std::collections::HashMap::new(),
            }
        }

        fn with_valid_token(mut self, token: &str, claims: TokenClaims) -> Self {
            self.valid_tokens.insert(token.to_string(), claims);
            self
        }
    }

    impl TokenService for MockTokenService {
        fn generate_access_token(
            &self,
            _user_id: &Uuid,
            _wallet_address: &str,
        ) -> Result<String, AuthError> {
            unimplemented!("Not needed for middleware tests")
        }

        fn generate_refresh_token(
            &self,
            _user_id: &Uuid,
            _wallet_address: &str,
        ) -> Result<String, AuthError> {
            unimplemented!("Not needed for middleware tests")
        }

        fn verify_access_token(&self, token: &str) -> Result<TokenClaims, AuthError> {
            self.valid_tokens
                .get(token)
                .cloned()
                .ok_or(AuthError::InvalidToken)
        }

        fn verify_refresh_token(&self, _token: &str) -> Result<TokenClaims, AuthError> {
            unimplemented!("Not needed for middleware tests")
        }
    }

    fn make_claims(user_id: Uuid, wallet: &str) -> TokenClaims {
        TokenClaims {
            sub: user_id.to_string(),
            wallet_address: wallet.to_string(),
            exp: (Utc::now().timestamp() + 3600) as i64,
            iat: Utc::now().timestamp() as i64,
            iss: "https://api.testudo.vip".to_string(),
            token_type: TokenType::Access,
        }
    }

    #[actix_web::test]
    async fn test_jwt_middleware_valid_token() {
        let user_id = Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        let mock = MockTokenService::new().with_valid_token("valid_token", make_claims(user_id, wallet));
        let token_service: Arc<dyn TokenService> = Arc::new(mock);

        let app = test::init_service(
            App::new()
                .wrap(JwtMiddleware::new(token_service))
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("authorization", "Bearer valid_token"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["user_id"], user_id.to_string());
        assert_eq!(body["wallet_address"], wallet);
    }

    #[actix_web::test]
    async fn test_jwt_middleware_cookie_extraction() {
        let user_id = Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        let mock = MockTokenService::new().with_valid_token("cookie_token", make_claims(user_id, wallet));
        let token_service: Arc<dyn TokenService> = Arc::new(mock);

        let app = test::init_service(
            App::new()
                .wrap(JwtMiddleware::new(token_service))
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .cookie(actix_web::cookie::Cookie::new("access_token", "cookie_token"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["user_id"], user_id.to_string());
    }

    #[actix_web::test]
    async fn test_bearer_takes_priority_over_cookie() {
        let user_id_bearer = Uuid::new_v4();
        let user_id_cookie = Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        let mock = MockTokenService::new()
            .with_valid_token("bearer_token", make_claims(user_id_bearer, wallet))
            .with_valid_token("cookie_token", make_claims(user_id_cookie, wallet));
        let token_service: Arc<dyn TokenService> = Arc::new(mock);

        let app = test::init_service(
            App::new()
                .wrap(JwtMiddleware::new(token_service))
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("authorization", "Bearer bearer_token"))
            .cookie(actix_web::cookie::Cookie::new("access_token", "cookie_token"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        // Bearer should win over cookie
        assert_eq!(body["user_id"], user_id_bearer.to_string());
    }

    #[actix_web::test]
    async fn test_jwt_middleware_invalid_token() {
        let mock = MockTokenService::new();
        let token_service: Arc<dyn TokenService> = Arc::new(mock);

        let app = test::init_service(
            App::new()
                .wrap(JwtMiddleware::new(token_service))
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("authorization", "Bearer invalid_token"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_jwt_middleware_missing_header() {
        let mock = MockTokenService::new();
        let token_service: Arc<dyn TokenService> = Arc::new(mock);

        let app = test::init_service(
            App::new()
                .wrap(JwtMiddleware::new(token_service))
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get().uri("/protected").to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[actix_web::test]
    async fn test_rate_limiting() {
        let mock = MockTokenService::new();
        let token_service: Arc<dyn TokenService> = Arc::new(mock);
        let middleware = JwtMiddleware::with_rate_limit(token_service, 2, Duration::from_secs(60));

        let app = test::init_service(
            App::new()
                .wrap(middleware)
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        for _ in 0..2 {
            let req = test::TestRequest::get()
                .uri("/protected")
                .insert_header(("authorization", "Bearer invalid_token"))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
        }

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("authorization", "Bearer invalid_token"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::TOO_MANY_REQUESTS
        );
    }

    #[actix_web::test]
    async fn test_security_headers() {
        let user_id = Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        let mock = MockTokenService::new().with_valid_token("valid_token", make_claims(user_id, wallet));
        let token_service: Arc<dyn TokenService> = Arc::new(mock);

        let app = test::init_service(
            App::new()
                .wrap(JwtMiddleware::new(token_service))
                .route("/protected", web::get().to(test_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("authorization", "Bearer valid_token"))
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let headers = resp.headers();
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert!(headers.get("content-security-policy").is_some());
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-xss-protection").unwrap(), "1; mode=block");
    }

    #[actix_web::test]
    async fn test_rate_limiter_basic() {
        let rate_limiter = RateLimiter::new(3, Duration::from_secs(60));
        let ip = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 1));

        for _ in 0..3 {
            assert!(rate_limiter.is_allowed(ip));
            rate_limiter.record_attempt(ip);
        }

        assert!(!rate_limiter.is_allowed(ip));
    }

    #[actix_web::test]
    async fn test_rate_limiter_time_window() {
        let rate_limiter = RateLimiter::new(2, Duration::from_millis(100));
        let ip = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 1));

        rate_limiter.record_attempt(ip);
        rate_limiter.record_attempt(ip);
        assert!(!rate_limiter.is_allowed(ip));

        std::thread::sleep(Duration::from_millis(150));
        assert!(rate_limiter.is_allowed(ip));
    }
}
