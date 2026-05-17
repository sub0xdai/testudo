//! ENG-01b — Public profile endpoint.
//!
//! GET /api/v1/public/profile/:handle — no auth required.
//! Per-IP rate limit: 60 req/min (dedicated limiter, separate from auth limiter).

use actix_web::{web, HttpRequest, HttpResponse, Result};
use std::net::IpAddr;

use crate::{
    middleware::RateLimiter,
    services::dignitas::handles::{HandleService, PublicProfileData, SparklinePoint},
    types::app::AppState,
};

pub async fn get_profile(
    req: HttpRequest,
    app_state: web::Data<AppState>,
    rate_limiter: web::Data<RateLimiter>,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let client_ip = req
        .connection_info()
        .peer_addr()
        .and_then(|s| s.parse().ok())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));

    if !rate_limiter.is_allowed(client_ip) {
        rate_limiter.record_attempt(client_ip);
        return Ok(HttpResponse::TooManyRequests().json(serde_json::json!({
            "code": "rate_limited",
            "message": "Too many requests",
        })));
    }
    rate_limiter.record_attempt(client_ip);

    let handle = path.into_inner();
    let svc = HandleService::new(app_state.pool.clone());

    match svc.get_public_profile(&handle).await {
        Ok(Some(profile)) => Ok(HttpResponse::Ok().json(profile)),
        Ok(None) => Ok(HttpResponse::NotFound().json(serde_json::json!({
            "code": "not_found",
            "message": "Handle not found",
        }))),
        Err(e) => {
            tracing::error!("public profile error for handle={}: {}", handle, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "code": "internal_error",
                "message": "Public profile request failed",
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::time::Duration;
    use uuid::Uuid;

    fn profile_fixture(show_score: bool, show_sparkline: bool) -> PublicProfileData {
        profile_fixture_full(show_score, show_sparkline, false)
    }

    fn profile_fixture_full(
        show_score: bool,
        show_sparkline: bool,
        show_streak: bool,
    ) -> PublicProfileData {
        PublicProfileData {
            handle: "testuser".to_string(),
            bio: Some("I trade the open".to_string()),
            member_since: Utc::now(),
            score: if show_score {
                Some(Decimal::new(7234, 2)) // "72.34"
            } else {
                None
            },
            sparkline: if show_sparkline {
                Some(vec![SparklinePoint {
                    date: chrono::NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                    score: Decimal::new(7000, 2),
                }])
            } else {
                None
            },
            streak_days: if show_streak { Some(47) } else { None },
            longest_ever: if show_streak { Some(92) } else { None },
            allow_indexing: false,
        }
    }

    // FR-10: all toggles off → both null (empty carcass)
    #[test]
    fn visibility_all_off_both_null() {
        let p = profile_fixture(false, false);
        assert!(p.score.is_none());
        assert!(p.sparkline.is_none());
    }

    // (b) show_score=true, show_sparkline=false
    #[test]
    fn visibility_score_on_sparkline_off() {
        let p = profile_fixture(true, false);
        assert!(p.score.is_some());
        assert!(p.sparkline.is_none());
    }

    // (c) show_score=false, show_sparkline=true
    #[test]
    fn visibility_score_off_sparkline_on() {
        let p = profile_fixture(false, true);
        assert!(p.score.is_none());
        assert!(p.sparkline.is_some());
    }

    // (d) both on
    #[test]
    fn visibility_both_on() {
        let p = profile_fixture(true, true);
        assert!(p.score.is_some());
        assert!(p.sparkline.is_some());
    }

    // ENG-01c: streak visibility is independent of score/sparkline toggles
    #[test]
    fn streak_visibility_independent() {
        let off = profile_fixture_full(true, true, false);
        assert!(off.streak_days.is_none());
        assert!(off.longest_ever.is_none());

        let on = profile_fixture_full(false, false, true);
        assert!(on.streak_days.is_some());
        assert!(on.longest_ever.is_some());
        assert!(on.score.is_none(), "streak on does not force score on");
    }

    // Rate-limit unit test (no DB required)
    #[test]
    fn rate_limit_60_per_min() {
        let limiter = RateLimiter::new(60, Duration::from_secs(60));
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        for _ in 0..60 {
            assert!(limiter.is_allowed(ip));
            limiter.record_attempt(ip);
        }
        // 61st is rejected
        assert!(!limiter.is_allowed(ip));
    }

    #[test]
    fn rate_limit_per_ip_independent() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        let ip1: IpAddr = "1.2.3.4".parse().unwrap();
        let ip2: IpAddr = "5.6.7.8".parse().unwrap();
        limiter.record_attempt(ip1);
        limiter.record_attempt(ip1);
        assert!(!limiter.is_allowed(ip1));
        assert!(limiter.is_allowed(ip2));
    }
}
