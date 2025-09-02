//! Authentication Provider for Testudo Trading Platform
//!
//! This module implements the authentication context provider following:
//! - SOP-001: Risk calculation verification with authenticated user context
//! - SOP-002: OODA loop integration requiring authenticated operations
//! - SOP-003: Recovery procedures with fallback authentication mechanisms
//!
//! Key security principles:
//! - JWT tokens stored in memory only (never localStorage per requirements)
//! - Automatic token refresh with graceful degradation
//! - Risk profile integration for Van Tharp calculations
//! - SOP-003 compliant recovery when authentication provider is unreachable

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Request, RequestInit, RequestMode, Response};

/// User authentication context matching backend UserClaims
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserContext {
    pub sub: String,
    pub preferred_username: String,
    pub email: String,
    pub name: String,
    pub risk_profile: RiskProfile,
    pub account_equity: f64,
    pub daily_loss_limit: f64,
    pub permissions: Vec<String>,
    pub iat: u64,
    pub exp: u64,
}

/// Risk profile classification matching backend RiskProfile enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RiskProfile {
    Conservative,
    Standard,
    Aggressive,
}

impl RiskProfile {
    pub fn max_trade_risk_percent(self) -> f64 {
        match self {
            RiskProfile::Conservative => 0.02, // 2%
            RiskProfile::Standard => 0.06,     // 6%
            RiskProfile::Aggressive => 0.10,   // 10%
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            RiskProfile::Conservative => "Conservative - Lower risk, smaller positions",
            RiskProfile::Standard => "Standard - Balanced Van Tharp methodology",
            RiskProfile::Aggressive => "Aggressive - Higher risk for experienced traders",
        }
    }
}

impl Default for RiskProfile {
    fn default() -> Self {
        RiskProfile::Standard
    }
}

/// Authentication state for reactive UI updates
#[derive(Debug, Clone, PartialEq)]
pub enum AuthState {
    /// Not authenticated, loading state unknown
    Unknown,
    /// Currently checking authentication status
    Loading,
    /// Successfully authenticated with user context
    Authenticated(UserContext),
    /// Authentication failed or session expired
    Unauthenticated,
    /// Authentication provider unreachable (SOP-003 scenario)
    ProviderUnreachable,
}

/// Authentication context provided to all components
#[derive(Clone, Copy)]
pub struct AuthContext {
    /// Current authentication state
    pub auth_state: ReadSignal<AuthState>,
    /// Trigger login redirect
    pub login: WriteSignal<()>,
    /// Trigger logout
    pub logout: WriteSignal<()>,
    /// Force refresh authentication status
    pub refresh: WriteSignal<()>,
}

/// Authentication provider component
/// 
/// This component manages the global authentication state and provides
/// context to all child components. It handles:
/// - Initial authentication check on app load
/// - Token refresh and session validation
/// - SOP-003 recovery procedures
/// - Memory-only token storage (never localStorage)
#[component]
pub fn AuthProvider(children: Children) -> impl IntoView {
    // Core authentication signals
    let (auth_state, set_auth_state) = signal(AuthState::Loading);
    let (login_trigger, set_login_trigger) = signal(());
    let (logout_trigger, set_logout_trigger) = signal(());
    let (refresh_trigger, set_refresh_trigger) = signal(());

    // Create authentication context
    let auth_context = AuthContext {
        auth_state,
        login: set_login_trigger,
        logout: set_logout_trigger,
        refresh: set_refresh_trigger,
    };

    // Provide context to all children
    provide_context(auth_context);

    // Handle login trigger
    create_effect(move |_| {
        login_trigger.track();
        spawn_local(async move {
            if let Err(e) = initiate_login().await {
                logging::error!("Login initiation failed: {}", e);
                set_auth_state.set(AuthState::Unauthenticated);
            }
        });
    });

    // Handle logout trigger  
    create_effect(move |_| {
        logout_trigger.track();
        spawn_local(async move {
            if let Err(e) = initiate_logout().await {
                logging::error!("Logout failed: {}", e);
            }
            set_auth_state.set(AuthState::Unauthenticated);
        });
    });

    // Handle refresh trigger and initial load
    create_effect(move |_| {
        refresh_trigger.track();
        spawn_local(async move {
            set_auth_state.set(AuthState::Loading);
            match check_authentication_status().await {
                Ok(Some(user_context)) => {
                    set_auth_state.set(AuthState::Authenticated(user_context));
                }
                Ok(None) => {
                    set_auth_state.set(AuthState::Unauthenticated);
                }
                Err(AuthError::ProviderUnreachable) => {
                    logging::warn!("Authentication provider unreachable - SOP-003 fallback");
                    set_auth_state.set(AuthState::ProviderUnreachable);
                }
                Err(e) => {
                    logging::error!("Authentication check failed: {}", e);
                    set_auth_state.set(AuthState::Unauthenticated);
                }
            }
        });
    });

    // Initial authentication check on component mount
    spawn_local(async move {
        set_refresh_trigger.set(());
    });

    view! {
        <div class="auth-provider">
            {children()}
        </div>
    }
}

/// Convenience hook to access authentication context
pub fn use_auth() -> AuthContext {
    use_context::<AuthContext>()
        .expect("AuthContext must be provided by AuthProvider")
}

/// Authentication errors with SOP-003 recovery classification
#[derive(Debug, Clone)]
pub enum AuthError {
    NetworkError(String),
    InvalidResponse(String),
    ProviderUnreachable,
    SessionExpired,
    InsufficientPermissions,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            AuthError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            AuthError::ProviderUnreachable => write!(f, "Authentication provider unreachable"),
            AuthError::SessionExpired => write!(f, "Session expired"),
            AuthError::InsufficientPermissions => write!(f, "Insufficient permissions"),
        }
    }
}

/// Initiate OAuth login flow by redirecting to backend auth endpoint
async fn initiate_login() -> Result<(), AuthError> {
    let window = web_sys::window().ok_or_else(|| {
        AuthError::NetworkError("Window not available".to_string())
    })?;

    let location = window.location();
    let current_url = location.href().map_err(|_| {
        AuthError::NetworkError("Failed to get current URL".to_string())
    })?;

    // Store return URL for post-login redirect (in sessionStorage for security)
    let storage = window.session_storage().map_err(|_| {
        AuthError::NetworkError("SessionStorage not available".to_string())
    })?.ok_or_else(|| {
        AuthError::NetworkError("SessionStorage not supported".to_string())
    })?;

    storage.set_item("auth_return_url", &current_url).map_err(|_| {
        AuthError::NetworkError("Failed to store return URL".to_string())
    })?;

    // Redirect to backend OAuth login endpoint
    let login_url = "/api/v1/auth/login";
    location.assign(login_url).map_err(|_| {
        AuthError::NetworkError("Failed to redirect to login".to_string())
    })?;

    Ok(())
}

/// Initiate logout by calling backend logout endpoint
async fn initiate_logout() -> Result<(), AuthError> {
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init("/api/v1/auth/logout", &opts)
        .map_err(|_| AuthError::NetworkError("Failed to create logout request".to_string()))?;

    let window = web_sys::window().ok_or_else(|| {
        AuthError::NetworkError("Window not available".to_string())
    })?;

    let resp_value = wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| AuthError::NetworkError("Logout request failed".to_string()))?;

    let _response: Response = resp_value.dyn_into().map_err(|_| {
        AuthError::InvalidResponse("Invalid logout response".to_string())
    })?;

    // Clear any session storage
    if let Ok(Some(storage)) = window.session_storage() {
        let _ = storage.remove_item("auth_return_url");
    }

    Ok(())
}

/// Check current authentication status with backend
async fn check_authentication_status() -> Result<Option<UserContext>, AuthError> {
    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init("/api/v1/auth/me", &opts)
        .map_err(|_| AuthError::NetworkError("Failed to create auth check request".to_string()))?;

    let window = web_sys::window().ok_or_else(|| {
        AuthError::NetworkError("Window not available".to_string())
    })?;

    let resp_value = match wasm_bindgen_futures::JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(resp) => resp,
        Err(_) => {
            // Network error could indicate provider unreachable (SOP-003)
            return Err(AuthError::ProviderUnreachable);
        }
    };

    let response: Response = resp_value.dyn_into().map_err(|_| {
        AuthError::InvalidResponse("Invalid auth check response".to_string())
    })?;

    let status = response.status();
    
    match status {
        200 => {
            // Successfully authenticated - parse user context
            let text_promise = response.text().map_err(|_| {
                AuthError::InvalidResponse("Failed to read response text".to_string())
            })?;
            
            let text = wasm_bindgen_futures::JsFuture::from(text_promise)
                .await
                .map_err(|_| AuthError::InvalidResponse("Failed to get response text".to_string()))?
                .as_string()
                .ok_or_else(|| AuthError::InvalidResponse("Response text not string".to_string()))?;

            // Parse JSON response
            let user_context: UserContext = serde_json::from_str(&text)
                .map_err(|e| AuthError::InvalidResponse(format!("JSON parse error: {}", e)))?;

            Ok(Some(user_context))
        }
        401 => {
            // Unauthorized - session expired or not authenticated
            Ok(None)
        }
        503 => {
            // Service unavailable - provider unreachable (SOP-003)
            Err(AuthError::ProviderUnreachable)
        }
        _ => {
            // Other error
            Err(AuthError::NetworkError(format!("Unexpected status: {}", status)))
        }
    }
}

/// Authentication status display component
#[component]
pub fn AuthStatus() -> impl IntoView {
    let auth = use_auth();
    
    view! {
        <div class="auth-status">
            {move || match auth.auth_state.get() {
                AuthState::Loading => view! {
                    <span class="status loading">"🔄 Checking authentication..."</span>
                }.into_view(),
                AuthState::Authenticated(user) => view! {
                    <div class="status authenticated">
                        <span class="user-info">
                            "👤 " {user.preferred_username.clone()}
                            <span class="risk-profile">" (" {user.risk_profile.description()} ")"</span>
                        </span>
                        <button 
                            class="logout-btn"
                            on:click=move |_| auth.logout.set(())
                        >
                            "Logout"
                        </button>
                    </div>
                }.into_view(),
                AuthState::Unauthenticated => view! {
                    <button 
                        class="login-btn"
                        on:click=move |_| auth.login.set(())
                    >
                        "🔐 Login"
                    </button>
                }.into_view(),
                AuthState::ProviderUnreachable => view! {
                    <div class="status provider-unreachable">
                        <span class="warning">"⚠️ Authentication provider unreachable"</span>
                        <button 
                            class="retry-btn"
                            on:click=move |_| auth.refresh.set(())
                        >
                            "Retry"
                        </button>
                    </div>
                }.into_view(),
                AuthState::Unknown => view! {
                    <span class="status unknown">"❓ Authentication status unknown"</span>
                }.into_view(),
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_profile_max_trade_risk_percent() {
        assert_eq!(RiskProfile::Conservative.max_trade_risk_percent(), 0.02);
        assert_eq!(RiskProfile::Standard.max_trade_risk_percent(), 0.06);
        assert_eq!(RiskProfile::Aggressive.max_trade_risk_percent(), 0.10);
    }

    #[test]
    fn test_risk_profile_default() {
        assert_eq!(RiskProfile::default(), RiskProfile::Standard);
    }

    #[test]
    fn test_user_context_serialization() {
        let user = UserContext {
            sub: "test-user-id".to_string(),
            preferred_username: "testuser".to_string(),
            email: "test@testudo.example".to_string(),
            name: "Test User".to_string(),
            risk_profile: RiskProfile::Standard,
            account_equity: 10000.0,
            daily_loss_limit: 500.0,
            permissions: vec!["trade:execute".to_string()],
            iat: 1640995200,
            exp: 1640995800,
        };

        let json = serde_json::to_string(&user).unwrap();
        let deserialized: UserContext = serde_json::from_str(&json).unwrap();
        assert_eq!(user, deserialized);
    }
}