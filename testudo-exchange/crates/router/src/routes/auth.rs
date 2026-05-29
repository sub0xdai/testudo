// @anchor exchange:router:auth
// @tags api

use actix_web::cookie::time::Duration as CookieDuration;
use actix_web::cookie::{Cookie, SameSite};
use actix_web::{web, HttpRequest, HttpResponse, Result};
use common_utils::auth::{hash_token, AuthError, REFRESH_TOKEN_EXPIRY_SECONDS};
use serde::Deserialize;

use crate::middleware::{AuthenticatedUser, RateLimiter};
use crate::repositories::session::{NewSession, SessionRepository};
use crate::repositories::user::PostgresUserRepository;
use crate::services::auth::{
    normalize_wallet_address, parse_siwe_message, parse_siws_message, validate_siwe_message,
    validate_siws_message, verify_siwe_signature, verify_siws_signature, NonceStore, PairingStore,
};
use crate::types::app::AppState;
use crate::types::auth::{ErrorResponse, UserResponse};

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SiweRequest {
    pub message: String,
    pub signature: String,
}

#[derive(Deserialize)]
pub struct VerifySiwsRequest {
    pub message: String,
    pub signature: String,
    pub address: String,
}

#[derive(Deserialize)]
pub struct PairRequest {
    pub code: String,
}

#[derive(Deserialize)]
pub struct ExtensionRefreshRequest {
    pub refresh_token: String,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Cookie domain for cross-origin subdomain auth (e.g., ".testudo.vip").
/// When set, cookies are sent across subdomains (desk.testudo.vip -> api.testudo.vip).
/// When unset (local dev), cookies are scoped to the current origin.
fn cookie_domain() -> Option<String> {
    std::env::var("COOKIE_DOMAIN").ok()
}

/// Build HttpOnly cookie pair for web/journal clients.
fn build_access_cookie(token: &str) -> Cookie<'static> {
    let mut cookie = Cookie::build("access_token", token.to_string())
        .http_only(true)
        .secure(true)
        .same_site(SameSite::None)
        .path("/api")
        .max_age(CookieDuration::seconds(900))
        .finish();
    if let Some(domain) = cookie_domain() {
        cookie.set_domain(domain);
    }
    cookie
}

fn build_refresh_cookie(token: &str) -> Cookie<'static> {
    let mut cookie = Cookie::build("refresh_token", token.to_string())
        .http_only(true)
        .secure(true)
        .same_site(SameSite::None)
        .path("/api/v1/auth")
        .max_age(CookieDuration::seconds(604_800))
        .finish();
    if let Some(domain) = cookie_domain() {
        cookie.set_domain(domain);
    }
    cookie
}

fn clear_access_cookie() -> Cookie<'static> {
    let mut cookie = Cookie::build("access_token", "")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::None)
        .path("/api")
        .max_age(CookieDuration::ZERO)
        .finish();
    if let Some(domain) = cookie_domain() {
        cookie.set_domain(domain);
    }
    cookie
}

fn clear_refresh_cookie() -> Cookie<'static> {
    let mut cookie = Cookie::build("refresh_token", "")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::None)
        .path("/api/v1/auth")
        .max_age(CookieDuration::ZERO)
        .finish();
    if let Some(domain) = cookie_domain() {
        cookie.set_domain(domain);
    }
    cookie
}

fn extract_client_ip(req: &HttpRequest) -> Option<String> {
    req.connection_info()
        .peer_addr()
        .map(|s| s.to_string())
}

fn extract_user_agent(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Create a session row and return (access_token, refresh_token).
async fn create_session_tokens(
    app_state: &AppState,
    session_repo: &SessionRepository,
    user_id: &uuid::Uuid,
    wallet_address: &str,
    req: &HttpRequest,
) -> Result<(String, String), AuthError> {
    let access = app_state
        .token_service
        .generate_access_token(user_id, wallet_address)?;
    let refresh = app_state
        .token_service
        .generate_refresh_token(user_id, wallet_address)?;

    let expires_at =
        chrono::Utc::now() + chrono::Duration::seconds(REFRESH_TOKEN_EXPIRY_SECONDS as i64);

    session_repo
        .create_session(NewSession {
            user_id: *user_id,
            refresh_token_hash: hash_token(&refresh),
            ip_address: extract_client_ip(req),
            user_agent: extract_user_agent(req),
            expires_at,
        })
        .await?;

    Ok((access, refresh))
}

fn auth_error_to_response(err: AuthError) -> HttpResponse {
    match &err {
        AuthError::InvalidToken | AuthError::TokenExpired | AuthError::WrongTokenType => {
            HttpResponse::Unauthorized().json(ErrorResponse::invalid_token())
        }
        AuthError::SessionRevoked => {
            HttpResponse::Unauthorized().json(ErrorResponse::new("session_revoked", err.to_string()))
        }
        AuthError::Unauthorized(_) => {
            HttpResponse::Unauthorized().json(ErrorResponse::new("unauthorized", err.to_string()))
        }
        AuthError::TokenGenerationFailed => {
            tracing::error!("Token generation failed");
            HttpResponse::InternalServerError().json(ErrorResponse::internal_error())
        }
        AuthError::RepositoryError(msg) => {
            tracing::error!("Repository error in auth: {}", msg);
            HttpResponse::InternalServerError().json(ErrorResponse::internal_error())
        }
    }
}

// ---------------------------------------------------------------------------
// GET /api/v1/auth/nonce
// ---------------------------------------------------------------------------

pub async fn get_nonce(nonce_store: web::Data<NonceStore>) -> Result<HttpResponse> {
    let nonce = nonce_store.generate();
    Ok(HttpResponse::Ok().json(serde_json::json!({ "nonce": nonce })))
}

// ---------------------------------------------------------------------------
// POST /api/v1/auth/verify-siwe
// ---------------------------------------------------------------------------

pub async fn verify_siwe(
    app_state: web::Data<AppState>,
    nonce_store: web::Data<NonceStore>,
    session_repo: web::Data<SessionRepository>,
    user_repo: web::Data<PostgresUserRepository>,
    body: web::Json<SiweRequest>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    // 1. Parse EIP-4361 message
    let parsed = match parse_siwe_message(&body.message) {
        Ok(p) => p,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 2. Validate nonce (consume = one-time use)
    let nonce_valid = nonce_store.consume(&parsed.nonce);

    // 3. Validate domain, version, expiration
    let expected_domain =
        std::env::var("SIWE_DOMAIN").unwrap_or_else(|_| "localhost:3001".to_string());
    if let Err(e) = validate_siwe_message(&parsed, &expected_domain, nonce_valid) {
        return Ok(auth_error_to_response(e));
    }

    // 4. Recover signer and verify address match
    let recovered = match verify_siwe_signature(&body.message, &body.signature) {
        Ok(addr) => addr,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 5. Find or create user
    let wallet_str = format!("{recovered}").to_lowercase();
    let user = match user_repo.find_or_create_by_wallet(&wallet_str).await {
        Ok(u) => u,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 6. Generate tokens + session
    let (access, refresh) = match create_session_tokens(
        &app_state,
        &session_repo,
        &user.id,
        &user.wallet_address,
        &req,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 7. Set HttpOnly cookies, return user info (NOT tokens)
    Ok(HttpResponse::Ok()
        .cookie(build_access_cookie(&access))
        .cookie(build_refresh_cookie(&refresh))
        .json(serde_json::json!({
            "user": UserResponse::from(user)
        })))
}

// ---------------------------------------------------------------------------
// POST /api/v1/auth/verify-siws
// ---------------------------------------------------------------------------

pub async fn verify_siws(
    app_state: web::Data<AppState>,
    nonce_store: web::Data<NonceStore>,
    session_repo: web::Data<SessionRepository>,
    user_repo: web::Data<PostgresUserRepository>,
    body: web::Json<VerifySiwsRequest>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    // 1. Parse SIWS message
    let parsed = match parse_siws_message(&body.message) {
        Ok(p) => p,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 2. Validate nonce (consume = one-time use)
    let nonce_valid = nonce_store.consume(&parsed.nonce);

    // 3. Validate domain and expiration (same SIWE_DOMAIN env var — it's the app domain)
    let expected_domain =
        std::env::var("SIWE_DOMAIN").unwrap_or_else(|_| "localhost:3001".to_string());
    if let Err(e) = validate_siws_message(&parsed, &expected_domain, nonce_valid) {
        return Ok(auth_error_to_response(e));
    }

    // 4. Verify Ed25519 signature
    if let Err(e) = verify_siws_signature(&body.message, &body.signature, &body.address) {
        return Ok(auth_error_to_response(e));
    }

    // 5. Ensure claimed address matches message address
    if parsed.address != body.address {
        return Ok(auth_error_to_response(AuthError::Unauthorized(
            "address mismatch: body address does not match message address".to_string(),
        )));
    }

    // 6. Normalize wallet address
    let wallet = match normalize_wallet_address(&body.address) {
        Ok(w) => w,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 7. Find or create user
    let user = match user_repo.find_or_create_by_wallet(&wallet).await {
        Ok(u) => u,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 8. Generate tokens + session
    let (access, refresh) = match create_session_tokens(
        &app_state,
        &session_repo,
        &user.id,
        &user.wallet_address,
        &req,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 9. Set HttpOnly cookies, return user info (identical to SIWE)
    Ok(HttpResponse::Ok()
        .cookie(build_access_cookie(&access))
        .cookie(build_refresh_cookie(&refresh))
        .json(serde_json::json!({
            "user": UserResponse::from(user)
        })))
}

// ---------------------------------------------------------------------------
// POST /api/v1/auth/refresh  (cookie-based for web/journal)
// ---------------------------------------------------------------------------

pub async fn refresh(
    app_state: web::Data<AppState>,
    session_repo: web::Data<SessionRepository>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    // Read refresh token from cookie
    let refresh_token = match req.cookie("refresh_token") {
        Some(c) => c.value().to_string(),
        None => {
            return Ok(HttpResponse::Unauthorized()
                .json(ErrorResponse::new("unauthorized", "Missing refresh token")))
        }
    };

    match rotate_refresh(
        &app_state,
        &session_repo,
        &refresh_token,
        &req,
    )
    .await
    {
        Ok((access, new_refresh, _user_response)) => Ok(HttpResponse::Ok()
            .cookie(build_access_cookie(&access))
            .cookie(build_refresh_cookie(&new_refresh))
            .json(serde_json::json!({ "user": _user_response }))),
        Err(e) => Ok(auth_error_to_response(e)),
    }
}

// ---------------------------------------------------------------------------
// POST /api/v1/auth/extension-refresh  (JSON body for extension)
// ---------------------------------------------------------------------------

pub async fn extension_refresh(
    app_state: web::Data<AppState>,
    session_repo: web::Data<SessionRepository>,
    body: web::Json<ExtensionRefreshRequest>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    match rotate_refresh(
        &app_state,
        &session_repo,
        &body.refresh_token,
        &req,
    )
    .await
    {
        Ok((access, new_refresh, _user_response)) => Ok(HttpResponse::Ok().json(
            serde_json::json!({
                "tokens": {
                    "access_token": access,
                    "refresh_token": new_refresh,
                    "expires_in": 900
                },
                "user": _user_response
            }),
        )),
        Err(e) => Ok(auth_error_to_response(e)),
    }
}

/// Shared refresh rotation logic — validates old token, revokes session, issues new tokens.
async fn rotate_refresh(
    app_state: &AppState,
    session_repo: &SessionRepository,
    refresh_token: &str,
    req: &HttpRequest,
) -> Result<(String, String, UserResponse), AuthError> {
    // 1. Verify JWT signature + type
    let claims = app_state.token_service.verify_refresh_token(refresh_token)?;

    // 2. Look up session by hash
    let token_hash = hash_token(refresh_token);
    let session = session_repo
        .find_by_token_hash(&token_hash)
        .await?
        .ok_or(AuthError::Unauthorized(
            "session not found".to_string(),
        ))?;

    // 3. Reject if revoked or expired
    if session.is_revoked {
        return Err(AuthError::SessionRevoked);
    }
    if session.expires_at < chrono::Utc::now() {
        return Err(AuthError::TokenExpired);
    }

    // 4. Revoke old session
    session_repo.revoke_session(session.id).await?;

    // 5. Generate new token pair + new session
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| AuthError::Unauthorized("invalid user_id in token".to_string()))?;
    let (access, new_refresh) =
        create_session_tokens(app_state, session_repo, &user_id, &claims.wallet_address, req)
            .await?;

    let user_response = UserResponse {
        id: user_id,
        wallet_address: claims.wallet_address,
    };

    Ok((access, new_refresh, user_response))
}

// ---------------------------------------------------------------------------
// POST /api/v1/auth/logout  (authenticated)
// ---------------------------------------------------------------------------

pub async fn logout(
    session_repo: web::Data<SessionRepository>,
    req: HttpRequest,
    body: Option<web::Json<ExtensionRefreshRequest>>,
) -> Result<HttpResponse> {
    // Path 1: Web/journal — revoke session via refresh cookie
    if let Some(refresh_cookie) = req.cookie("refresh_token") {
        let token_hash = hash_token(refresh_cookie.value());
        if let Ok(Some(session)) = session_repo.find_by_token_hash(&token_hash).await {
            let _ = session_repo.revoke_session(session.id).await;
        }
    }

    // Path 2: Extension — revoke session via refresh token in JSON body
    if let Some(body) = body {
        let token_hash = hash_token(&body.refresh_token);
        if let Ok(Some(session)) = session_repo.find_by_token_hash(&token_hash).await {
            let _ = session_repo.revoke_session(session.id).await;
        }
    }

    Ok(HttpResponse::Ok()
        .cookie(clear_access_cookie())
        .cookie(clear_refresh_cookie())
        .json(serde_json::json!({ "message": "Logged out" })))
}

// ---------------------------------------------------------------------------
// POST /api/v1/auth/revoke-all  (authenticated)
// ---------------------------------------------------------------------------

pub async fn revoke_all(
    user: AuthenticatedUser,
    session_repo: web::Data<SessionRepository>,
) -> Result<HttpResponse> {
    let revoked = match session_repo.revoke_all_for_user(user.user_id).await {
        Ok(n) => n,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    Ok(HttpResponse::Ok()
        .cookie(clear_access_cookie())
        .cookie(clear_refresh_cookie())
        .json(serde_json::json!({
            "message": "All sessions revoked",
            "revoked_count": revoked
        })))
}

// ---------------------------------------------------------------------------
// GET /api/v1/auth/me  (authenticated)
// ---------------------------------------------------------------------------

pub async fn me(user: AuthenticatedUser) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "user": {
            "id": user.user_id,
            "wallet_address": user.wallet_address,
        }
    })))
}

// ---------------------------------------------------------------------------
// POST /api/v1/auth/pair-extension  (authenticated — web generates code)
// ---------------------------------------------------------------------------

pub async fn pair_extension(
    user: AuthenticatedUser,
    pairing_store: web::Data<PairingStore>,
) -> Result<HttpResponse> {
    let code = pairing_store.generate(user.user_id);
    Ok(HttpResponse::Ok().json(serde_json::json!({ "code": code })))
}

// ---------------------------------------------------------------------------
// GET /api/v1/auth/pair-status  (authenticated — web polls for completion)
// ---------------------------------------------------------------------------

pub async fn pair_status(
    user: AuthenticatedUser,
    pairing_store: web::Data<PairingStore>,
) -> Result<HttpResponse> {
    let paired = pairing_store.check_paired(&user.user_id);
    Ok(HttpResponse::Ok().json(serde_json::json!({ "paired": paired })))
}

// ---------------------------------------------------------------------------
// POST /api/v1/auth/extension-pair  (unauthenticated — extension sends code)
// ---------------------------------------------------------------------------

pub async fn extension_pair(
    app_state: web::Data<AppState>,
    session_repo: web::Data<SessionRepository>,
    user_repo: web::Data<PostgresUserRepository>,
    pairing_store: web::Data<PairingStore>,
    pair_rate_limiter: web::Data<RateLimiter>,
    body: web::Json<PairRequest>,
    req: HttpRequest,
) -> Result<HttpResponse> {
    // Rate limit: 5 attempts per minute per IP to prevent brute-force on 6-digit codes
    let ip = req
        .connection_info()
        .peer_addr()
        .and_then(|s| s.parse().ok())
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
    if !pair_rate_limiter.is_allowed(ip) {
        return Ok(
            HttpResponse::TooManyRequests().json(ErrorResponse::new(
                "rate_limited",
                "Too many pairing attempts. Try again later.",
            )),
        );
    }

    // 1. Consume pairing code (one-time use)
    let user_id = match pairing_store.take(&body.code) {
        Some(id) => id,
        None => {
            return Ok(HttpResponse::Unauthorized().json(ErrorResponse::new(
                "unauthorized",
                "Invalid or expired pairing code",
            )))
        }
    };

    // 2. Look up user
    let user = match user_repo.find_by_id(&user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Ok(HttpResponse::Unauthorized().json(ErrorResponse::new(
                "unauthorized",
                "User not found",
            )))
        }
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 3. Generate tokens + session
    let (access, refresh) = match create_session_tokens(
        &app_state,
        &session_repo,
        &user.id,
        &user.wallet_address,
        &req,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => return Ok(auth_error_to_response(e)),
    };

    // 4. JSON body (not cookies) — extension stores in chrome.storage.session
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "tokens": {
            "access_token": access,
            "refresh_token": refresh,
            "expires_in": 900
        },
        "user": UserResponse::from(user)
    })))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test as actix_test, web, App};
    use common_utils::auth::{AuthError, JwtTokenService, TokenClaims, TokenService, TokenType};
    use crate::middleware::JwtMiddleware;
    use std::sync::Arc;

    fn test_token_service() -> Arc<dyn TokenService> {
        Arc::new(JwtTokenService::new(
            "access_secret_that_is_long_enough_for_testing_purposes_here".to_string(),
            "refresh_secret_that_is_long_enough_for_testing_purposes_here".to_string(),
        ))
    }

    // --- GET /nonce ---

    #[actix_web::test]
    async fn test_get_nonce_returns_nonce() {
        let nonce_store = web::Data::new(NonceStore::new());
        let app = actix_test::init_service(
            App::new()
                .app_data(nonce_store.clone())
                .route("/nonce", web::get().to(get_nonce)),
        )
        .await;

        let req = actix_test::TestRequest::get().uri("/nonce").to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        let nonce = body["nonce"].as_str().unwrap();
        assert_eq!(nonce.len(), 32);
        assert!(nonce.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[actix_web::test]
    async fn test_get_nonce_unique_per_call() {
        let nonce_store = web::Data::new(NonceStore::new());
        let app = actix_test::init_service(
            App::new()
                .app_data(nonce_store.clone())
                .route("/nonce", web::get().to(get_nonce)),
        )
        .await;

        let req1 = actix_test::TestRequest::get().uri("/nonce").to_request();
        let resp1 = actix_test::call_service(&app, req1).await;
        let body1: serde_json::Value = actix_test::read_body_json(resp1).await;

        let req2 = actix_test::TestRequest::get().uri("/nonce").to_request();
        let resp2 = actix_test::call_service(&app, req2).await;
        let body2: serde_json::Value = actix_test::read_body_json(resp2).await;

        assert_ne!(body1["nonce"], body2["nonce"]);
    }

    // --- GET /me (via JwtMiddleware with real token) ---

    #[actix_web::test]
    async fn test_me_returns_user_info() {
        let token_service = test_token_service();
        let user_id = uuid::Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        let access_token = token_service
            .generate_access_token(&user_id, wallet)
            .unwrap();

        let app = actix_test::init_service(
            App::new().service(
                web::scope("")
                    .wrap(JwtMiddleware::new(token_service.clone()))
                    .route("/me", web::get().to(me)),
            ),
        )
        .await;

        let req = actix_test::TestRequest::get()
            .uri("/me")
            .insert_header(("authorization", format!("Bearer {access_token}")))
            .to_request();

        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        // /me wraps response under "user" key: {"user": {"id": ..., "wallet_address": ...}}
        assert_eq!(body["user"]["id"], user_id.to_string());
        assert_eq!(body["user"]["wallet_address"], wallet);
    }

    // --- Cookie helpers ---

    #[test]
    fn test_build_access_cookie_properties() {
        let cookie = build_access_cookie("test_token");
        assert_eq!(cookie.name(), "access_token");
        assert_eq!(cookie.value(), "test_token");
        assert!(cookie.http_only().unwrap_or(false));
        assert!(cookie.secure().unwrap_or(false));
        assert_eq!(cookie.same_site(), Some(SameSite::None));
        assert_eq!(cookie.path(), Some("/api"));
        assert_eq!(cookie.max_age(), Some(CookieDuration::seconds(900)));
    }

    #[test]
    fn test_build_refresh_cookie_properties() {
        let cookie = build_refresh_cookie("test_refresh");
        assert_eq!(cookie.name(), "refresh_token");
        assert_eq!(cookie.value(), "test_refresh");
        assert!(cookie.http_only().unwrap_or(false));
        assert!(cookie.secure().unwrap_or(false));
        assert_eq!(cookie.same_site(), Some(SameSite::None));
        assert_eq!(cookie.path(), Some("/api/v1/auth"));
        assert_eq!(cookie.max_age(), Some(CookieDuration::seconds(604_800)));
    }

    #[test]
    fn test_clear_cookies_zero_max_age() {
        let access = clear_access_cookie();
        assert_eq!(access.max_age(), Some(CookieDuration::ZERO));
        assert_eq!(access.value(), "");

        let refresh = clear_refresh_cookie();
        assert_eq!(refresh.max_age(), Some(CookieDuration::ZERO));
        assert_eq!(refresh.value(), "");
    }

    // --- Error mapping ---

    #[test]
    fn test_auth_error_mapping_unauthorized() {
        let resp = auth_error_to_response(AuthError::Unauthorized("bad sig".to_string()));
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_mapping_invalid_token() {
        let resp = auth_error_to_response(AuthError::InvalidToken);
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_mapping_session_revoked() {
        let resp = auth_error_to_response(AuthError::SessionRevoked);
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_mapping_token_expired() {
        let resp = auth_error_to_response(AuthError::TokenExpired);
        assert_eq!(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_auth_error_mapping_generation_failed() {
        let resp = auth_error_to_response(AuthError::TokenGenerationFailed);
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_auth_error_mapping_repository_error() {
        let resp = auth_error_to_response(AuthError::RepositoryError("db down".to_string()));
        assert_eq!(
            resp.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    // --- Pair extension (code generation via middleware) ---

    #[actix_web::test]
    async fn test_pair_extension_returns_6_digit_code() {
        let pairing_store = web::Data::new(PairingStore::new());
        let token_service = test_token_service();
        let user_id = uuid::Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        let access_token = token_service
            .generate_access_token(&user_id, wallet)
            .unwrap();

        let app = actix_test::init_service(
            App::new()
                .app_data(pairing_store.clone())
                .service(
                    web::scope("")
                        .wrap(JwtMiddleware::new(token_service.clone()))
                        .route("/pair", web::post().to(pair_extension)),
                ),
        )
        .await;

        let req = actix_test::TestRequest::post()
            .uri("/pair")
            .insert_header(("authorization", format!("Bearer {access_token}")))
            .to_request();

        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        let code = body["code"].as_str().unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    // --- Pair status (polling for completion via middleware) ---

    #[actix_web::test]
    async fn test_pair_status_returns_false_before_pairing() {
        let pairing_store = web::Data::new(PairingStore::new());
        let token_service = test_token_service();
        let user_id = uuid::Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        let access_token = token_service
            .generate_access_token(&user_id, wallet)
            .unwrap();

        let app = actix_test::init_service(
            App::new()
                .app_data(pairing_store.clone())
                .service(
                    web::scope("")
                        .wrap(JwtMiddleware::new(token_service.clone()))
                        .route("/pair-status", web::get().to(pair_status)),
                ),
        )
        .await;

        let req = actix_test::TestRequest::get()
            .uri("/pair-status")
            .insert_header(("authorization", format!("Bearer {access_token}")))
            .to_request();

        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["paired"], false);
    }

    #[actix_web::test]
    async fn test_pair_status_returns_true_after_pairing() {
        let pairing_store = web::Data::new(PairingStore::new());
        let token_service = test_token_service();
        let user_id = uuid::Uuid::new_v4();
        let wallet = "0xC285000000000000000000000000000000005b36";

        // Generate and redeem a pairing code
        let code = pairing_store.generate(user_id);
        pairing_store.take(&code);

        let access_token = token_service
            .generate_access_token(&user_id, wallet)
            .unwrap();

        let app = actix_test::init_service(
            App::new()
                .app_data(pairing_store.clone())
                .service(
                    web::scope("")
                        .wrap(JwtMiddleware::new(token_service.clone()))
                        .route("/pair-status", web::get().to(pair_status)),
                ),
        )
        .await;

        let req = actix_test::TestRequest::get()
            .uri("/pair-status")
            .insert_header(("authorization", format!("Bearer {access_token}")))
            .to_request();

        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["paired"], true);
    }

    // --- Extension pair (invalid code — direct store test) ---

    #[test]
    fn test_extension_pair_invalid_code_rejected() {
        let pairing_store = PairingStore::new();
        assert!(pairing_store.take("000000").is_none());
    }

    #[test]
    fn test_extension_pair_code_is_one_time_use() {
        let pairing_store = PairingStore::new();
        let user_id = uuid::Uuid::new_v4();
        let code = pairing_store.generate(user_id);
        assert_eq!(pairing_store.take(&code), Some(user_id));
        assert_eq!(pairing_store.take(&code), None); // second attempt fails
    }

    // --- UserResponse conversion ---

    #[test]
    fn test_user_response_serialization() {
        let resp = UserResponse {
            id: uuid::Uuid::nil(),
            wallet_address: "0xdead".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["wallet_address"], "0xdead");
    }
}
