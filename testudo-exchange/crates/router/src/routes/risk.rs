//! Risk Snapshot Routes (RSK-01)
//!
//! GET /api/v1/risk/snapshot — unified live risk view across every connected venue.

use actix_web::{web, HttpResponse, Result};
use serde::Serialize;

use crate::{
    middleware::AuthenticatedUser,
    services::risk_snapshot::{self, RiskError},
    types::app::AppState,
};

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

/// GET /api/v1/risk/snapshot
///
/// Returns the aggregated [`RiskSnapshot`] for the authenticated user.
pub async fn get_snapshot(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> Result<HttpResponse> {
    match risk_snapshot::build_snapshot(user.user_id, &app_state).await {
        Ok(snapshot) => Ok(HttpResponse::Ok().json(snapshot)),
        Err(RiskError::Internal(msg)) => {
            tracing::error!(
                "Failed to build risk snapshot for user {}: {}",
                user.user_id,
                msg
            );
            Ok(HttpResponse::InternalServerError().json(ErrorBody {
                code: "risk_snapshot_failed",
                message: "Failed to build risk snapshot".to_string(),
            }))
        }
    }
}
