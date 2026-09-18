//! TS-01 — TypeSafe judgement routes.
//!
//! All endpoints require JWT auth (wired via `JwtMiddleware` in `main.rs`).
//!
//! # Response shape
//!
//! Every response is the envelope `{ "data": ... }` where `data` is nullable.
//! A judgement that is unavailable is `data: null`, never an HTTP error. A 503
//! would force the client to handle a network failure for what is a normal and
//! expected degradation, and the client's contract is to show nothing.
//!
//! Design and scope: `docs/plans/typesafe-jev-sniper-integration.md`.

// @anchor exchange:router:judgment
// @tags api

use actix_web::{web, HttpResponse, Result};
use common_utils::journal::TradeSide;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    middleware::AuthenticatedUser,
    services::journal_service::fetch_setup_tags,
    services::typesafe::{resolve_setup_tag, TagCandidate, TagResolution, TagResolutionInput},
    types::{app::AppState, auth::ErrorResponse},
};

/// Tags loaded as resolution candidates.
///
/// Larger than the 20 the tag picker shows: candidate generation drops tags
/// that share no overlap with what was typed, so a wider net costs nothing
/// when it is filtered before the request is built.
const KNOWN_TAG_LIMIT: i64 = 40;

/// Upper bound on the typed tag forwarded.
///
/// A setup tag is a label, not prose. This is the trust boundary for text
/// that is about to reach a third party.
const MAX_TYPED_TAG_LEN: usize = 120;

/// Upper bound on the symbol and timeframe forwarded.
const MAX_CONTEXT_LEN: usize = 40;

/// Trade side as the client sends it.
///
/// A closed set, so serde rejects anything else with a 400 before the handler
/// runs. No string parsing and no default.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SideIn {
    Long,
    Short,
}

impl From<SideIn> for TradeSide {
    fn from(side: SideIn) -> Self {
        match side {
            SideIn::Long => Self::Long,
            SideIn::Short => Self::Short,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PreTradeRequest {
    /// What the trader typed into the setup tag field.
    pub typed_tag: String,
    pub symbol: String,
    pub side: SideIn,
    pub timeframe: String,
}

/// A resolved setup tag. Mirrors [`TagResolution`] without its internal
/// reasons, which are logged and never sent to a client.
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum SetupTagJudgment {
    /// Write this tag verbatim: calibration finds history with
    /// `LOWER(setup_tag) = LOWER($2)`.
    Matched { tag: String, confidence: Decimal },
    /// Keep what the trader typed.
    Novel,
}

#[derive(Debug, Serialize)]
pub struct PreTradeResponse {
    /// `None` when the integration is off, unreachable, or answered
    /// unusably. The client shows nothing and behaves as before.
    pub data: Option<SetupTagJudgment>,
}

fn internal_error(err: impl std::fmt::Display) -> HttpResponse {
    tracing::error!("judgment route error: {}", err);
    HttpResponse::InternalServerError().json(ErrorResponse::new(
        "judgment_internal",
        "Judgement request failed",
    ))
}

/// Maps a resolution onto the wire shape.
///
/// `Unavailable` becomes a null payload, which is the approved decision: the
/// client degrades silently and never has to branch on an error for a
/// judgement it did not need. The inner reason is logged, not sent.
fn to_wire(resolution: TagResolution) -> PreTradeResponse {
    let data = match resolution {
        TagResolution::Matched { tag, confidence } => {
            Some(SetupTagJudgment::Matched { tag, confidence })
        }
        TagResolution::KeepAsTyped => Some(SetupTagJudgment::Novel),
        TagResolution::Unavailable { .. } => None,
    };
    PreTradeResponse { data }
}

// ─────────────────────────────────────────────────────────────────────────
// POST /api/v1/judgment/pre-trade
// ─────────────────────────────────────────────────────────────────────────

/// UC-1. Resolves a free-text setup tag onto one of the trader's own tags.
///
/// Never fails the request on a judgement problem: an unreachable model, a
/// rate limit, or a flat answer all produce `data: null`, and the modal keeps
/// the trader's own text.
pub async fn pre_trade(
    app_state: web::Data<AppState>,
    user: AuthenticatedUser,
    body: web::Json<PreTradeRequest>,
) -> Result<HttpResponse> {
    let typed = body.typed_tag.trim();
    if typed.chars().count() > MAX_TYPED_TAG_LEN {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "tag_too_long",
            format!("typed_tag must be at most {MAX_TYPED_TAG_LEN} characters"),
        )));
    }
    if body.symbol.chars().count() > MAX_CONTEXT_LEN
        || body.timeframe.chars().count() > MAX_CONTEXT_LEN
    {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse::new(
            "context_too_long",
            format!("symbol and timeframe must be at most {MAX_CONTEXT_LEN} characters"),
        )));
    }

    // The trader's own vocabulary. A failure here is a read failure, not a
    // judgement failure, so it surfaces instead of looking like "no history":
    // silently reporting Novel would hide a broken query behind a plausible
    // answer.
    let known = match fetch_setup_tags(&app_state.pool, user.user_id, KNOWN_TAG_LIMIT).await {
        Ok(entries) => entries
            .into_iter()
            .map(|entry| TagCandidate {
                tag: entry.name,
                // COUNT(*) is i64. A negative or oversized value would be a
                // database fault, so clamp rather than wrap.
                uses: u32::try_from(entry.uses).unwrap_or(u32::MAX),
            })
            .collect(),
        Err(e) => return Ok(internal_error(e)),
    };

    let input = TagResolutionInput {
        typed_tag: typed.to_string(),
        known_tags: known,
        symbol: body.symbol.clone(),
        side: body.side.into(),
        timeframe: body.timeframe.clone(),
    };

    let data = resolve_setup_tag(app_state.typesafe_client.as_deref(), &input).await;

    Ok(HttpResponse::Ok().json(to_wire(data)))
}

// ─────────────────────────────────────────────────────────────────────────
// GET /api/v1/health/typesafe
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TypeSafeHealth {
    /// Whether a client exists. `false` means every judgement returns
    /// `data: null`, which is the expected state until the credential and the
    /// privacy copy are in place.
    pub enabled: bool,
    /// The pinned model id, present only when enabled. No endpoint, no
    /// credential, and no key material is reported.
    pub model: Option<String>,
}

/// Lets an operator confirm the integration state without fabricating a trade.
pub async fn health(app_state: web::Data<AppState>) -> Result<HttpResponse> {
    let (enabled, model) = match app_state.typesafe_client.as_ref() {
        Some(client) => (true, Some(client.model().to_string())),
        None => (false, None),
    };
    Ok(HttpResponse::Ok().json(TypeSafeHealth { enabled, model }))
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;
    use crate::services::typesafe::TypeSafeError;

    #[test]
    fn unavailable_is_a_null_payload_and_not_an_error() {
        let body = to_wire(TagResolution::Unavailable {
            reason: Some(TypeSafeError::RateLimit { retry_after: None }),
        });
        let value = serde_json::to_value(&body).expect("response serialises");
        assert!(value["data"].is_null());
        // No error surface at all: the client must not have to branch on one.
        assert!(value.get("error").is_none());
        assert!(value.get("message").is_none());
    }

    #[test]
    fn a_matched_resolution_carries_the_tag_and_its_confidence() {
        let body = to_wire(TagResolution::Matched {
            tag: "breakout".to_string(),
            confidence: dec!(0.88),
        });
        let value = serde_json::to_value(&body).expect("response serialises");
        assert_eq!(value["data"]["outcome"], "matched");
        assert_eq!(value["data"]["tag"], "breakout");
        // Decimal's JSON spelling is the serializer's business, so assert a
        // round trip instead of pinning a format the client would inherit.
        let confidence: Decimal = serde_json::from_value(value["data"]["confidence"].clone())
            .expect("confidence must round-trip");
        assert_eq!(confidence, dec!(0.88));
    }

    #[test]
    fn a_novel_resolution_is_an_outcome_not_an_absence() {
        // "keep what the trader typed" is a decision; "unavailable" is not.
        // Collapsing them would make a resolved novel tag indistinguishable
        // from an unreachable model.
        let value = serde_json::to_value(to_wire(TagResolution::KeepAsTyped))
            .expect("response serialises");
        assert_eq!(value["data"]["outcome"], "novel");
        assert!(!value["data"].is_null());
    }

    #[test]
    fn side_in_accepts_only_the_uppercase_closed_set() {
        assert!(matches!(
            serde_json::from_value::<SideIn>(json!("LONG")),
            Ok(SideIn::Long)
        ));
        assert!(matches!(
            serde_json::from_value::<SideIn>(json!("SHORT")),
            Ok(SideIn::Short)
        ));
        for bad in [json!("long"), json!("sideways"), json!(""), json!(1)] {
            assert!(
                serde_json::from_value::<SideIn>(bad.clone()).is_err(),
                "{bad} must be rejected at the boundary"
            );
        }
    }
}
