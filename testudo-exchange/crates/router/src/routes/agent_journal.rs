//! AGENT-03: Agent journal memory endpoints.
//!
//! Three endpoints for agent-facing journal queries:
//! - GET /journal/agent/summary   — consolidated performance summary (JSON + LLM markdown)
//! - GET /journal/agent/insights  — pattern detection from coach pipeline
//! - POST /journal/agent/compare  — period-over-period comparison
//!
//! All endpoints require JWT authentication (SIWE bearer token).

// @anchor exchange:router:agent_journal
// @tags api

use actix_web::{web, HttpResponse};

use crate::{
    middleware::AuthenticatedUser,
    models::agent_journal::{AgentSummaryQuery, SummaryFormat},
    policy::{Action, ActionContext},
    services::agent_journal::AgentJournalService,
    types::app::AppState,
};

// ─────────────────────────────────────────────────────────────────────────
// GET /journal/agent/summary
// ─────────────────────────────────────────────────────────────────────────

pub async fn get_summary(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    query: web::Query<AgentSummaryQuery>,
) -> HttpResponse {
    if let Err(e) = user.authorize(Action::JournalRead, &ActionContext::default()) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "code": "insufficient_permissions",
            "message": format!("{:?}", e),
        }));
    }

    let service = AgentJournalService::new(
        app_state.pool.clone(),
        app_state.analytics_pool.clone(),
    );

    match service.build_summary(user.user_id, &query).await {
        Ok(summary) => {
            if query.format == SummaryFormat::Llm {
                let md = crate::services::agent_journal_formatter::format_summary_llm(&summary);
                HttpResponse::Ok()
                    .content_type("text/markdown; charset=utf-8")
                    .body(md)
            } else {
                HttpResponse::Ok().json(summary)
            }
        }
        Err(e) => {
            tracing::error!(
                user_id = %user.user_id,
                error = %e,
                "agent journal summary failed"
            );
            HttpResponse::InternalServerError().json(serde_json::json!({
                "code": "agent_journal_internal",
                "message": "Failed to compute agent journal summary"
            }))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// GET /journal/agent/insights (placeholder — implemented in CP-4)
// ─────────────────────────────────────────────────────────────────────────

pub async fn get_insights(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
) -> HttpResponse {
    if let Err(e) = user.authorize(Action::JournalRead, &ActionContext::default()) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "code": "insufficient_permissions",
            "message": format!("{:?}", e),
        }));
    }

    let service = AgentJournalService::new(
        app_state.pool.clone(),
        app_state.analytics_pool.clone(),
    );

    let insights = service
        .build_insights(user.user_id, &app_state.coach_service)
        .await;

    HttpResponse::Ok().json(serde_json::json!({
        "insights": insights,
        "total": insights.len()
    }))
}

// ─────────────────────────────────────────────────────────────────────────
// POST /journal/agent/compare (placeholder — implemented in CP-5)
// ─────────────────────────────────────────────────────────────────────────

pub async fn post_compare(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<crate::models::agent_journal::CompareRequest>,
) -> HttpResponse {
    if let Err(e) = user.authorize(Action::JournalRead, &ActionContext::default()) {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "code": "insufficient_permissions",
            "message": format!("{:?}", e),
        }));
    }

    let request = body.into_inner();

    // Validate date ranges.
    if request.period_a.from > request.period_a.to
        || request.period_b.from > request.period_b.to
    {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "code": "invalid_date_range",
            "message": "from date must be before to date"
        }));
    }

    let service = AgentJournalService::new(
        app_state.pool.clone(),
        app_state.analytics_pool.clone(),
    );

    match service.build_comparison(user.user_id, &request).await {
        Ok(result) => HttpResponse::Ok().json(result),
        Err(e) => {
            tracing::error!(
                user_id = %user.user_id,
                error = %e,
                "agent journal comparison failed"
            );
            HttpResponse::InternalServerError().json(serde_json::json!({
                "code": "agent_journal_internal",
                "message": "Failed to compute comparison"
            }))
        }
    }
}
