//! Citation grounding validator.
//!
//! Hard gate between narrator output and persistence. A `NarratedReport`
//! is only accepted when **every** claim it makes cites a trade that is
//! present in the digest's `flagged_trades`. Two grounding checks run:
//!
//! 1. Each `NarrativeSection.citations` UUID must appear in
//!    `digest.flagged_trades.*.id`.
//! 2. Every `[T-xxxxxxxx]` token in the headline and in every section body
//!    must match a `short_id` on `digest.flagged_trades`.
//!
//! When validation fails, the scheduler (T7) persists a stats-only fallback
//! row instead of discarding the work — the user still sees the
//! deterministic pattern stats with `● coach unavailable this week` in the
//! narrative slot.
//!
//! The token regex is compiled once via `lazy_static!` (pattern matches
//! `[T-` + exactly 8 lowercase hex characters + `]`, consistent with the
//! `short_id` format produced by `digest::make_short_id`).

// @anchor exchange:router:validator
// @tags api

use std::collections::HashSet;

use lazy_static::lazy_static;
use regex::Regex;

use super::types::{CoachDigest, NarratedReport, ValidationError};

lazy_static! {
    static ref CITATION_TOKEN_RE: Regex =
        Regex::new(r"\[T-([0-9a-f]{8})\]").expect("citation token regex is valid");
}

/// Verify that every citation (UUID list + `[T-xxx]` tokens) in the
/// narrated report resolves to a trade present in the digest.
pub fn validate(report: &NarratedReport, digest: &CoachDigest) -> Result<(), ValidationError> {
    let known_ids: HashSet<uuid::Uuid> = digest.flagged_trades.iter().map(|t| t.id).collect();
    let known_short_ids: HashSet<&str> = digest
        .flagged_trades
        .iter()
        .map(|t| t.short_id.as_str())
        .collect();

    check_tokens(&report.headline, &known_short_ids, None, "headline")?;

    for (idx, section) in report.sections.iter().enumerate() {
        for trade_id in &section.citations {
            if !known_ids.contains(trade_id) {
                return Err(ValidationError::UnknownCitation {
                    section_index: idx,
                    trade_id: *trade_id,
                });
            }
        }

        let location = format!("section[{idx}].body");
        check_tokens(&section.body, &known_short_ids, Some(idx), &location)?;
    }

    Ok(())
}

fn check_tokens(
    text: &str,
    known_short_ids: &HashSet<&str>,
    section_index: Option<usize>,
    location: &str,
) -> Result<(), ValidationError> {
    for capture in CITATION_TOKEN_RE.captures_iter(text) {
        let token = capture
            .get(1)
            .expect("regex guarantees capture group 1")
            .as_str();
        if !known_short_ids.contains(token) {
            return Err(ValidationError::UnknownToken {
                section_index,
                token: token.to_string(),
                location: location.to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use uuid::Uuid;

    use super::super::types::{
        CoachDigest, FlaggedPattern, NarrativeSection, NarratedReport, PatternKind, Severity,
        TradeEvidence, UserBaseline, WeekStats,
    };
    use super::*;

    fn make_short_id(id: Uuid) -> String {
        id.simple().to_string().chars().take(8).collect()
    }

    fn trade(id: Uuid, symbol: &str) -> TradeEvidence {
        let opened = Utc.with_ymd_and_hms(2026, 4, 13, 14, 0, 0).unwrap();
        TradeEvidence {
            id,
            short_id: make_short_id(id),
            symbol: symbol.to_string(),
            side: "long".to_string(),
            opened_at: opened,
            closed_at: opened + chrono::Duration::hours(1),
            pnl: dec!(10),
            r_multiple: Some(dec!(1)),
            setup_tag: None,
            position_size_usd: dec!(1000),
        }
    }

    fn empty_baseline() -> UserBaseline {
        UserBaseline {
            avg_trades_per_day: dec!(1),
            avg_position_size_usd: dec!(1000),
            typical_session_hours_utc: vec![13, 14, 15, 16],
            win_rate: dec!(0.5),
            avg_r_multiple: dec!(1),
            p90_trades_per_6h: dec!(2),
            setup_baselines: HashMap::new(),
        }
    }

    fn empty_stats() -> WeekStats {
        WeekStats {
            trade_count: 0,
            win_rate: Decimal::ZERO,
            total_pnl: Decimal::ZERO,
            total_r: Decimal::ZERO,
            trades_by_hour_utc: [0; 24],
            by_setup: HashMap::new(),
        }
    }

    fn digest_with(trades: Vec<TradeEvidence>) -> CoachDigest {
        let user_id = Uuid::new_v4();
        let week_start = Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap();
        CoachDigest {
            user_id,
            week_start,
            week_end: week_start + chrono::Duration::days(7),
            baseline: empty_baseline(),
            week_stats: empty_stats(),
            flagged_patterns: vec![FlaggedPattern {
                pattern: PatternKind::SizingDrift,
                severity: Severity::Notable,
                evidence: trades.iter().map(|t| t.id).collect(),
                metrics: serde_json::json!({}),
            }],
            flagged_trades: trades,
        }
    }

    fn base_report(sections: Vec<NarrativeSection>, headline: &str) -> NarratedReport {
        NarratedReport {
            headline: headline.to_string(),
            sections,
            model_used: "mock".to_string(),
            cache_hit_ratio: None,
            generated_at: Utc::now(),
        }
    }

    #[test]
    fn accepts_report_with_all_citations_present() {
        let t1 = trade(Uuid::new_v4(), "BTC_USDT");
        let t2 = trade(Uuid::new_v4(), "ETH_USDT");
        let body = format!(
            "You sized up after losing on [T-{}] then doubled again on [T-{}].",
            t1.short_id, t2.short_id
        );
        let headline = format!("Sizing drift on [T-{}]", t1.short_id);
        let report = base_report(
            vec![NarrativeSection {
                pattern: PatternKind::SizingDrift,
                body,
                citations: vec![t1.id, t2.id],
            }],
            &headline,
        );
        let digest = digest_with(vec![t1, t2]);

        validate(&report, &digest).expect("valid report should pass");
    }

    #[test]
    fn rejects_citation_uuid_not_in_digest() {
        let t1 = trade(Uuid::new_v4(), "BTC_USDT");
        let stranger = Uuid::new_v4();
        let report = base_report(
            vec![NarrativeSection {
                pattern: PatternKind::SizingDrift,
                body: "no tokens here".to_string(),
                citations: vec![stranger],
            }],
            "headline without tokens",
        );
        let digest = digest_with(vec![t1]);

        match validate(&report, &digest) {
            Err(ValidationError::UnknownCitation {
                section_index,
                trade_id,
            }) => {
                assert_eq!(section_index, 0);
                assert_eq!(trade_id, stranger);
            }
            other => panic!("expected UnknownCitation, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_token_in_section_body() {
        let t1 = trade(Uuid::new_v4(), "BTC_USDT");
        // Token "deadbeef" is 8 hex chars but does not match t1.short_id.
        let body = format!(
            "Reference to known [T-{}] and stranger [T-deadbeef].",
            t1.short_id
        );
        let report = base_report(
            vec![NarrativeSection {
                pattern: PatternKind::SizingDrift,
                body,
                citations: vec![t1.id],
            }],
            "headline without tokens",
        );
        let digest = digest_with(vec![t1]);

        match validate(&report, &digest) {
            Err(ValidationError::UnknownToken {
                section_index,
                token,
                location,
            }) => {
                assert_eq!(section_index, Some(0));
                assert_eq!(token, "deadbeef");
                assert_eq!(location, "section[0].body");
            }
            other => panic!("expected UnknownToken, got {other:?}"),
        }
    }

    #[test]
    fn rejects_unknown_token_in_headline() {
        let t1 = trade(Uuid::new_v4(), "BTC_USDT");
        let headline = "Stranger cite [T-cafebabe] right in the headline".to_string();
        let report = base_report(
            vec![NarrativeSection {
                pattern: PatternKind::SizingDrift,
                body: "no tokens here".to_string(),
                citations: vec![t1.id],
            }],
            &headline,
        );
        let digest = digest_with(vec![t1]);

        match validate(&report, &digest) {
            Err(ValidationError::UnknownToken {
                section_index,
                token,
                location,
            }) => {
                assert_eq!(section_index, None);
                assert_eq!(token, "cafebabe");
                assert_eq!(location, "headline");
            }
            other => panic!("expected UnknownToken, got {other:?}"),
        }
    }

    #[test]
    fn ignores_malformed_tokens_not_matching_regex() {
        // `[T-abc]` is 3 chars — not 8 hex — so regex doesn't capture it.
        // `[T-AAAAAAAA]` uses uppercase — regex is lowercase-only — skipped.
        let t1 = trade(Uuid::new_v4(), "BTC_USDT");
        let body = format!(
            "Malformed [T-abc] or [T-AAAAAAAA] — only [T-{}] counts.",
            t1.short_id
        );
        let report = base_report(
            vec![NarrativeSection {
                pattern: PatternKind::SizingDrift,
                body,
                citations: vec![t1.id],
            }],
            "headline without tokens",
        );
        let digest = digest_with(vec![t1]);

        validate(&report, &digest).expect("malformed tokens should be ignored");
    }

    #[test]
    fn report_with_no_sections_and_no_tokens_passes() {
        let report = base_report(vec![], "All quiet this week.");
        let digest = digest_with(vec![]);

        validate(&report, &digest).expect("empty report should pass");
    }
}
