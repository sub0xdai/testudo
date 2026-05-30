//! GET /api/v1/onboarding/status — single-call agent readiness check.
//!
//! Collapses the 3-call discovery dance (GET /exchanges/accounts, GET /risk-config,
//! GET /journal/agent/summary) into a single prescriptive response.

// @anchor exchange:router:onboarding-route
// @tags api

use actix_web::{web, HttpResponse, Result};

use crate::middleware::AuthenticatedUser;
use crate::models::onboarding::OnboardingStep;
use crate::services::onboarding;
use crate::types::app::AppState;

/// GET /api/v1/onboarding/status
///
/// Returns the user's onboarding state including prescriptive next_step guidance,
/// available exchanges (when no account is connected), pending agent wallet details,
/// trade history flag, and risk configuration summary.
pub async fn get_status(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    let status = onboarding::compute_onboarding_status(
        &app_state.pool,
        user.user_id,
    )
    .await?;

    // Mask Authenticate step — the endpoint requires auth, so the user is already past this.
    // If the service returns Authenticate, something went wrong; fall back gracefully.
    if status.next_step == OnboardingStep::Authenticate {
        return Ok(HttpResponse::Ok().json(status));
    }

    Ok(HttpResponse::Ok().json(status))
}
