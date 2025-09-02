//! Protected Route Component for Testudo Trading Platform
//!
//! This module implements route protection with:
//! - Authentication requirement enforcement
//! - Risk profile validation for trading operations
//! - Permission-based access control
//! - SOP compliance for secure trading operations

use leptos::prelude::*;
use super::{use_auth, AuthState, RiskProfile, UserContext};

/// Required permissions for different route types
#[derive(Debug, Clone, PartialEq)]
pub enum RoutePermission {
    /// Read-only access to account information
    ViewAccount,
    /// Execute trades (requires trade:execute permission)
    ExecuteTrades,
    /// Admin access to platform settings
    AdminAccess,
    /// Any authenticated user
    Authenticated,
}

impl RoutePermission {
    /// Get the backend permission string for this route permission
    pub fn permission_string(&self) -> Option<&'static str> {
        match self {
            RoutePermission::ViewAccount => None, // All authenticated users
            RoutePermission::ExecuteTrades => Some("trade:execute"),
            RoutePermission::AdminAccess => Some("admin:access"),
            RoutePermission::Authenticated => None,
        }
    }

    /// Check if user has the required permission
    pub fn check_permission(&self, user: &UserContext) -> bool {
        match self.permission_string() {
            Some(required_perm) => user.permissions.contains(&required_perm.to_string()),
            None => true, // No specific permission required
        }
    }
}

/// Minimum risk profile requirements for certain operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MinimumRiskProfile {
    /// Any risk profile allowed
    Any,
    /// Standard or Aggressive profiles required
    Standard,
    /// Only Aggressive profile allowed
    Aggressive,
}

impl MinimumRiskProfile {
    /// Check if user's risk profile meets the minimum requirement
    pub fn check_risk_profile(&self, user_profile: RiskProfile) -> bool {
        match (self, user_profile) {
            (MinimumRiskProfile::Any, _) => true,
            (MinimumRiskProfile::Standard, RiskProfile::Conservative) => false,
            (MinimumRiskProfile::Standard, _) => true,
            (MinimumRiskProfile::Aggressive, RiskProfile::Aggressive) => true,
            (MinimumRiskProfile::Aggressive, _) => false,
        }
    }
}

/// Protected route component that enforces authentication and permissions
#[component]
pub fn ProtectedRoute(
    /// Children components to render when access is granted
    children: Children,
    /// Required permission level for this route
    #[prop(default = RoutePermission::Authenticated)]
    required_permission: RoutePermission,
    /// Minimum risk profile required (for trading routes)
    #[prop(default = MinimumRiskProfile::Any)]
    minimum_risk_profile: MinimumRiskProfile,
    /// Custom redirect path for unauthorized access
    #[prop(optional)]
    unauthorized_redirect: Option<String>,
    /// Show loading spinner during auth check
    #[prop(default = true)]
    show_loading: bool,
) -> impl IntoView {
    let auth = use_auth();

    view! {
        <div class="protected-route">
            {move || {
                match auth.auth_state.get() {
                    AuthState::Loading if show_loading => view! {
                        <div class="auth-loading">
                            <div class="spinner">
                                <div class="testudo-spinner"></div>
                            </div>
                            <p>"🛡️ Verifying authentication..."</p>
                        </div>
                    }.into_view(),
                    
                    AuthState::Loading => view! {
                        <div style="display: none;"></div>
                    }.into_view(),
                    
                    AuthState::Authenticated(user) => {
                        // Check permissions
                        if !required_permission.check_permission(&user) {
                            return view! {
                                <AccessDenied 
                                    reason="insufficient_permissions"
                                    required_permission=required_permission.clone()
                                    user=user.clone()
                                />
                            }.into_view();
                        }

                        // Check risk profile requirements
                        if !minimum_risk_profile.check_risk_profile(user.risk_profile) {
                            return view! {
                                <AccessDenied 
                                    reason="insufficient_risk_profile"
                                    required_permission=required_permission.clone()
                                    user=user.clone()
                                />
                            }.into_view();
                        }

                        // All checks passed - render children
                        children().into_view()
                    },
                    
                    AuthState::Unauthenticated => view! {
                        <UnauthenticatedAccess 
                            required_permission=required_permission.clone()
                            redirect_path=unauthorized_redirect.clone()
                        />
                    }.into_view(),
                    
                    AuthState::ProviderUnreachable => view! {
                        <ProviderUnreachableError />
                    }.into_view(),
                    
                    AuthState::Unknown => view! {
                        <div class="auth-unknown">
                            <p>"⚠️ Authentication status unknown"</p>
                            <button 
                                class="retry-btn"
                                on:click=move |_| auth.refresh.set(())
                            >
                                "Retry Authentication"
                            </button>
                        </div>
                    }.into_view(),
                }
            }}
        </div>
    }
}

/// Component shown when access is denied due to insufficient permissions or risk profile
#[component]
fn AccessDenied(
    reason: &'static str,
    required_permission: RoutePermission,
    user: UserContext,
) -> impl IntoView {
    view! {
        <div class="access-denied">
            <div class="error-content">
                <h2>"🚫 Access Denied"</h2>
                {match reason {
                    "insufficient_permissions" => view! {
                        <div class="permission-error">
                            <p>"You do not have the required permissions to access this page."</p>
                            <div class="requirement-details">
                                <p><strong>"Required:"</strong> {
                                    match required_permission {
                                        RoutePermission::ExecuteTrades => "Execute Trades Permission",
                                        RoutePermission::AdminAccess => "Administrator Access",
                                        RoutePermission::ViewAccount => "Account View Access",
                                        RoutePermission::Authenticated => "Authenticated User",
                                    }
                                }</p>
                                <p><strong>"Your permissions:"</strong> {
                                    if user.permissions.is_empty() {
                                        "None".to_string()
                                    } else {
                                        user.permissions.join(", ")
                                    }
                                }</p>
                            </div>
                            <p class="help-text">
                                "Contact your account administrator to request the necessary permissions."
                            </p>
                        </div>
                    }.into_view(),
                    "insufficient_risk_profile" => view! {
                        <div class="risk-profile-error">
                            <p>"Your current risk profile does not meet the requirements for this trading operation."</p>
                            <div class="requirement-details">
                                <p><strong>"Your risk profile:"</strong> {user.risk_profile.description()}</p>
                                <p><strong>"Maximum trade risk:"</strong> {format!("{:.1}%", user.risk_profile.max_trade_risk_percent() * 100.0)}</p>
                            </div>
                            <p class="help-text">
                                "High-risk trading operations may require a Standard or Aggressive risk profile. "
                                "Contact support to discuss upgrading your risk classification."
                            </p>
                        </div>
                    }.into_view(),
                    _ => view! {
                        <p>"Access denied for unknown reason."</p>
                    }.into_view(),
                }}
                
                <div class="actions">
                    <button 
                        class="back-btn"
                        on:click=move |_| {
                            if let Some(window) = web_sys::window() {
                                if let Ok(history) = window.history() {
                                    let _ = history.back();
                                }
                            }
                        }
                    >
                        "← Go Back"
                    </button>
                </div>
            </div>
        </div>
    }
}

/// Component shown when user is not authenticated
#[component]
fn UnauthenticatedAccess(
    required_permission: RoutePermission,
    redirect_path: Option<String>,
) -> impl IntoView {
    let auth = use_auth();

    view! {
        <div class="unauthenticated-access">
            <div class="login-prompt">
                <h2>"🔐 Authentication Required"</h2>
                <p>"You must be logged in to access this page."</p>
                
                {match required_permission {
                    RoutePermission::ExecuteTrades => view! {
                        <div class="trading-warning">
                            <p class="warning-text">
                                "⚠️ This is a <strong>trading interface</strong> that requires secure authentication."
                            </p>
                            <p>"Please log in with your verified Testudo trading account."</p>
                        </div>
                    }.into_view(),
                    _ => view! {
                        <p>"Please log in to continue."</p>
                    }.into_view(),
                }}
                
                <div class="actions">
                    <button 
                        class="login-btn primary"
                        on:click=move |_| auth.login.set(())
                    >
                        "🔐 Login to Testudo"
                    </button>
                    
                    {redirect_path.map(|path| view! {
                        <p class="redirect-info">
                            "After logging in, you will be redirected to: " 
                            <code>{path}</code>
                        </p>
                    })}
                </div>
            </div>
        </div>
    }
}

/// Component shown when authentication provider is unreachable (SOP-003 scenario)
#[component]
fn ProviderUnreachableError() -> impl IntoView {
    let auth = use_auth();

    view! {
        <div class="provider-unreachable">
            <div class="error-content">
                <h2>"⚠️ Authentication Service Unavailable"</h2>
                <div class="sop-notice">
                    <p><strong>"SOP-003 Recovery Mode"</strong></p>
                    <p>"The authentication provider is currently unreachable. This is a temporary condition."</p>
                </div>
                
                <div class="recovery-options">
                    <h3>"Recovery Actions:"</h3>
                    <ul>
                        <li>"Check your network connection"</li>
                        <li>"Verify the authentication service status"</li>
                        <li>"Contact system administrators if the issue persists"</li>
                    </ul>
                </div>
                
                <div class="actions">
                    <button 
                        class="retry-btn"
                        on:click=move |_| auth.refresh.set(())
                    >
                        "🔄 Retry Connection"
                    </button>
                </div>
                
                <div class="technical-info">
                    <details>
                        <summary>"Technical Details"</summary>
                        <p>
                            "Following Standard Operating Procedure SOP-003, the system has "
                            "detected an authentication provider outage. Normal service will "
                            "resume once connectivity is restored."
                        </p>
                    </details>
                </div>
            </div>
        </div>
    }
}

/// Convenience hook to check if user is authenticated
pub fn use_is_authenticated() -> ReadSignal<bool> {
    let auth = use_auth();
    create_memo(move |_| {
        matches!(auth.auth_state.get(), AuthState::Authenticated(_))
    }).into()
}

/// Convenience hook to get authenticated user context
pub fn use_user() -> ReadSignal<Option<UserContext>> {
    let auth = use_auth();
    create_memo(move |_| {
        match auth.auth_state.get() {
            AuthState::Authenticated(user) => Some(user),
            _ => None,
        }
    }).into()
}

/// Convenience hook to check if user has specific permission
pub fn use_has_permission(permission: RoutePermission) -> ReadSignal<bool> {
    let user = use_user();
    create_memo(move |_| {
        match user.get() {
            Some(user) => permission.check_permission(&user),
            None => false,
        }
    }).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_permission_check() {
        let user = UserContext {
            sub: "test-user".to_string(),
            preferred_username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
            risk_profile: RiskProfile::Standard,
            account_equity: 10000.0,
            daily_loss_limit: 500.0,
            permissions: vec!["trade:execute".to_string()],
            iat: 1640995200,
            exp: 1640995800,
        };

        assert!(RoutePermission::Authenticated.check_permission(&user));
        assert!(RoutePermission::ViewAccount.check_permission(&user));
        assert!(RoutePermission::ExecuteTrades.check_permission(&user));
        assert!(!RoutePermission::AdminAccess.check_permission(&user));
    }

    #[test]
    fn test_minimum_risk_profile_check() {
        assert!(MinimumRiskProfile::Any.check_risk_profile(RiskProfile::Conservative));
        assert!(MinimumRiskProfile::Any.check_risk_profile(RiskProfile::Standard));
        assert!(MinimumRiskProfile::Any.check_risk_profile(RiskProfile::Aggressive));

        assert!(!MinimumRiskProfile::Standard.check_risk_profile(RiskProfile::Conservative));
        assert!(MinimumRiskProfile::Standard.check_risk_profile(RiskProfile::Standard));
        assert!(MinimumRiskProfile::Standard.check_risk_profile(RiskProfile::Aggressive));

        assert!(!MinimumRiskProfile::Aggressive.check_risk_profile(RiskProfile::Conservative));
        assert!(!MinimumRiskProfile::Aggressive.check_risk_profile(RiskProfile::Standard));
        assert!(MinimumRiskProfile::Aggressive.check_risk_profile(RiskProfile::Aggressive));
    }
}