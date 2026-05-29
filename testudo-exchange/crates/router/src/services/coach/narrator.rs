//! Narrator trait + OpenAI-compatible implementation.
//!
//! Prod impl: [`OpenAiNarrator`] — wraps `async-openai`'s `Client` pointed at
//! any OpenAI-compatible endpoint (DeepSeek-V3 by default, GLM/OpenRouter/OpenAI
//! also work). The system-role prompt is `include_str!`-ed from `prompts/system.md`
//! so the bytes are identical across requests; that stable prefix is what
//! earns the provider's prompt-cache hit.
//!
//! Test impl: [`MockNarrator`] — returns a pre-configured `Result` so
//! `CoachService`'s happy-path, narrator-failure, and validation-failure
//! branches can all be unit-tested without an HTTP dependency.
//!
//! The LLM is only asked to generate the narrative content (`headline` +
//! `sections`). Metadata fields on [`NarratedReport`] — `model_used`,
//! `cache_hit_ratio`, `generated_at` — are filled in by the narrator after
//! the call so the LLM can't lie about them.

// @anchor exchange:router:narrator
// @tags api

use std::sync::Mutex;

use async_openai::config::OpenAIConfig;
use async_openai::error::OpenAIError;
use async_openai::types::chat::{
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, CompletionUsage, CreateChatCompletionRequestArgs,
};
use async_openai::Client;
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::Deserialize;

use super::types::{CoachDigest, NarratedReport, NarrativeSection, NarratorError};

/// Static system-role prompt. Byte-stable across all requests so the
/// provider's prompt-cache can hit on consecutive runs. The per-user
/// digest payload is the only part that varies.
pub(super) const SYSTEM_PROMPT: &str = include_str!("prompts/system.md");

/// Produces a structured narrative from a digest. Object-safe.
#[async_trait]
pub trait Narrator: Send + Sync {
    async fn narrate(&self, digest: &CoachDigest) -> Result<NarratedReport, NarratorError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// OpenAI-compatible production narrator
// ─────────────────────────────────────────────────────────────────────────────

/// Wraps `async-openai`'s `Client` with a custom base URL so any
/// OpenAI-compatible provider works (DeepSeek, GLM, OpenRouter, OpenAI itself).
pub struct OpenAiNarrator {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiNarrator {
    /// Constructs a narrator pointed at `base_url` with `api_key`, using
    /// `model` for chat completions.
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let config = OpenAIConfig::new()
            .with_api_base(base_url.into())
            .with_api_key(api_key.into());
        Self {
            client: Client::with_config(config),
            model: model.into(),
        }
    }
}

/// Inner schema the LLM is asked to return. Metadata fields on
/// [`NarratedReport`] are not in the LLM's output so it can't forge them.
#[derive(Debug, Deserialize)]
struct LlmNarrativeContent {
    headline: String,
    sections: Vec<NarrativeSection>,
}

/// Convert an `async-openai` error into a typed `NarratorError`. Timeouts
/// and connect errors come through `OpenAIError::Reqwest`; rate limits and
/// other server-side failures come through `OpenAIError::ApiError`.
fn map_openai_error(err: OpenAIError) -> NarratorError {
    match err {
        OpenAIError::Reqwest(e) => {
            if e.is_timeout() {
                NarratorError::Timeout
            } else {
                NarratorError::Provider(format!("network error: {e}"))
            }
        }
        OpenAIError::ApiError(api_err) => {
            // Rate limits surface as HTTP 429. `ApiError.code` is a string
            // slot the provider can use for machine-readable categories.
            let is_rate_limit = api_err
                .code
                .as_deref()
                .map(|c| c.eq_ignore_ascii_case("rate_limit_exceeded") || c == "429")
                .unwrap_or(false)
                || api_err.message.to_lowercase().contains("rate limit");
            if is_rate_limit {
                NarratorError::RateLimit
            } else {
                NarratorError::Provider(api_err.message)
            }
        }
        OpenAIError::JSONDeserialize(e, _raw) => {
            NarratorError::Parse(format!("response shape mismatch: {e}"))
        }
        other => NarratorError::Provider(other.to_string()),
    }
}

/// Best-effort cache-hit ratio extraction. Providers that expose
/// `prompt_tokens_details.cached_tokens` (OpenAI convention) return a
/// ratio in `[0, 1]`. Providers that don't (some DeepSeek/GLM revisions
/// surface non-standard fields) return `None` — cache metrics log as
/// empty in that case.
fn extract_cache_hit_ratio(usage: Option<&CompletionUsage>) -> Option<Decimal> {
    let usage = usage?;
    let prompt_total = usage.prompt_tokens;
    if prompt_total == 0 {
        return None;
    }
    let cached = usage
        .prompt_tokens_details
        .as_ref()
        .and_then(|d| d.cached_tokens)?;
    // `Decimal::from(u32) / Decimal::from(u32)` — exact within decimal
    // precision bounds for typical token counts.
    Some(Decimal::from(cached) / Decimal::from(prompt_total))
}

#[async_trait]
impl Narrator for OpenAiNarrator {
    async fn narrate(&self, digest: &CoachDigest) -> Result<NarratedReport, NarratorError> {
        let digest_json = serde_json::to_string(digest)
            .map_err(|e| NarratorError::Parse(format!("digest serialize failed: {e}")))?;

        let system_msg = ChatCompletionRequestSystemMessageArgs::default()
            .content(SYSTEM_PROMPT)
            .build()
            .map_err(map_openai_error)?;
        let user_msg = ChatCompletionRequestUserMessageArgs::default()
            .content(digest_json)
            .build()
            .map_err(map_openai_error)?;

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(vec![
                ChatCompletionRequestMessage::System(system_msg),
                ChatCompletionRequestMessage::User(user_msg),
            ])
            .build()
            .map_err(map_openai_error)?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .map_err(map_openai_error)?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| NarratorError::Parse("response had no choices".into()))?;
        let content = choice
            .message
            .content
            .ok_or_else(|| NarratorError::Parse("response message had no content".into()))?;

        let narrative: LlmNarrativeContent = serde_json::from_str(&content)
            .map_err(|e| NarratorError::Parse(format!("LLM output not valid JSON: {e}")))?;

        let cache_hit_ratio = extract_cache_hit_ratio(response.usage.as_ref());

        Ok(NarratedReport {
            headline: narrative.headline,
            sections: narrative.sections,
            model_used: self.model.clone(),
            cache_hit_ratio,
            generated_at: Utc::now(),
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Test-only narrator
// ─────────────────────────────────────────────────────────────────────────────

/// Test double. Hand it a single `Result` up front; `narrate` returns it
/// (consuming it — a second call panics so tests fail loudly if the
/// system-under-test invokes the narrator twice unexpectedly).
pub struct MockNarrator {
    response: Mutex<Option<Result<NarratedReport, NarratorError>>>,
}

impl MockNarrator {
    pub fn new(response: Result<NarratedReport, NarratorError>) -> Self {
        Self {
            response: Mutex::new(Some(response)),
        }
    }
}

#[async_trait]
impl Narrator for MockNarrator {
    async fn narrate(&self, _digest: &CoachDigest) -> Result<NarratedReport, NarratorError> {
        self.response
            .lock()
            .expect("MockNarrator mutex poisoned")
            .take()
            .expect("MockNarrator::narrate called more than once")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests (inline — router is a binary-only crate, no external tests/ dir)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::TimeZone;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    use super::super::types::{
        CoachDigest, NarratedReport, NarrativeSection, NarratorError, PatternKind, UserBaseline,
        WeekStats,
    };
    use super::*;

    fn empty_digest() -> CoachDigest {
        CoachDigest {
            user_id: Uuid::nil(),
            week_start: Utc.with_ymd_and_hms(2026, 4, 13, 0, 0, 0).unwrap(),
            week_end: Utc.with_ymd_and_hms(2026, 4, 20, 0, 0, 0).unwrap(),
            baseline: UserBaseline {
                avg_trades_per_day: Decimal::ZERO,
                avg_position_size_usd: Decimal::ZERO,
                typical_session_hours_utc: vec![],
                win_rate: Decimal::ZERO,
                avg_r_multiple: Decimal::ZERO,
                p90_trades_per_6h: Decimal::ZERO,
                setup_baselines: HashMap::new(),
            },
            week_stats: WeekStats {
                trade_count: 0,
                win_rate: Decimal::ZERO,
                total_pnl: Decimal::ZERO,
                total_r: Decimal::ZERO,
                trades_by_hour_utc: [0; 24],
                by_setup: HashMap::new(),
            },
            flagged_patterns: vec![],
            flagged_trades: vec![],
        }
    }

    fn sample_report() -> NarratedReport {
        NarratedReport {
            headline: "Size climbed after losses this week.".into(),
            sections: vec![NarrativeSection {
                pattern: PatternKind::SizingDrift,
                body: "Three post-loss trades [T-a1b2c3d4] doubled the baseline.".into(),
                citations: vec![Uuid::nil()],
            }],
            model_used: "mock".into(),
            cache_hit_ratio: None,
            generated_at: Utc.with_ymd_and_hms(2026, 4, 20, 18, 0, 0).unwrap(),
        }
    }

    #[tokio::test]
    async fn mock_narrator_returns_configured_ok_response() {
        let narrator = MockNarrator::new(Ok(sample_report()));
        let got = narrator.narrate(&empty_digest()).await.expect("ok response");
        assert_eq!(got.headline, "Size climbed after losses this week.");
        assert_eq!(got.sections.len(), 1);
        assert_eq!(got.sections[0].pattern, PatternKind::SizingDrift);
    }

    #[tokio::test]
    async fn mock_narrator_propagates_timeout_error() {
        let narrator = MockNarrator::new(Err(NarratorError::Timeout));
        let err = narrator
            .narrate(&empty_digest())
            .await
            .expect_err("should error");
        assert!(matches!(err, NarratorError::Timeout));
    }

    #[tokio::test]
    async fn mock_narrator_propagates_parse_error() {
        let narrator = MockNarrator::new(Err(NarratorError::Parse("bad json".into())));
        let err = narrator
            .narrate(&empty_digest())
            .await
            .expect_err("should error");
        match err {
            NarratorError::Parse(msg) => assert_eq!(msg, "bad json"),
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn mock_narrator_propagates_rate_limit_and_provider_errors() {
        let narrator = MockNarrator::new(Err(NarratorError::RateLimit));
        assert!(matches!(
            narrator.narrate(&empty_digest()).await,
            Err(NarratorError::RateLimit)
        ));

        let narrator = MockNarrator::new(Err(NarratorError::Provider("500 boom".into())));
        match narrator.narrate(&empty_digest()).await {
            Err(NarratorError::Provider(msg)) => assert_eq!(msg, "500 boom"),
            other => panic!("expected Provider, got {other:?}"),
        }
    }

    #[test]
    fn llm_narrative_content_parses_headline_and_sections() {
        let uid = "11111111-2222-3333-4444-555555555555";
        let raw = format!(
            r#"{{
                "headline": "Sizing drift + streak risk flagged.",
                "sections": [
                    {{
                        "pattern": "sizing_drift",
                        "body": "Post-loss sizing spiked [T-{short}].",
                        "citations": ["{uid}"]
                    }}
                ]
            }}"#,
            short = &uid[..8]
        );

        let parsed: LlmNarrativeContent =
            serde_json::from_str(&raw).expect("valid LLM JSON should parse");
        assert_eq!(parsed.headline, "Sizing drift + streak risk flagged.");
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].pattern, PatternKind::SizingDrift);
        assert_eq!(parsed.sections[0].citations.len(), 1);
        assert_eq!(
            parsed.sections[0].citations[0].to_string(),
            uid.to_string()
        );
    }

    #[test]
    fn llm_narrative_content_rejects_unknown_pattern_kind() {
        let raw = r#"{
            "headline": "foo",
            "sections": [
                { "pattern": "made_up_pattern", "body": "x", "citations": [] }
            ]
        }"#;
        let err = serde_json::from_str::<LlmNarrativeContent>(raw)
            .expect_err("unknown pattern should fail deserialization");
        // serde reports the invalid variant; any parse error is acceptable,
        // we just need the error path wired.
        assert!(err.to_string().to_lowercase().contains("unknown"));
    }

    #[test]
    fn llm_narrative_content_rejects_malformed_json() {
        let err = serde_json::from_str::<LlmNarrativeContent>("{not json")
            .expect_err("malformed JSON must fail");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn system_prompt_is_bundled_and_substantive() {
        // include_str! guarantees the file is present at compile time.
        // The ≥ 4 KB floor is the spec's acceptance criterion.
        assert!(
            SYSTEM_PROMPT.len() >= 4_000,
            "system prompt too short ({} bytes)",
            SYSTEM_PROMPT.len()
        );
        // Sanity-check key content so an accidental blank file would fail here.
        assert!(SYSTEM_PROMPT.contains("Testudo Coach"));
        assert!(SYSTEM_PROMPT.contains("sizing_drift"));
        assert!(SYSTEM_PROMPT.contains("correlation_stack"));
        assert!(SYSTEM_PROMPT.contains("[T-"));
    }

    #[test]
    fn extract_cache_hit_ratio_handles_present_and_missing_metadata() {
        use async_openai::types::chat::{
            CompletionTokensDetails, CompletionUsage, PromptTokensDetails,
        };

        let usage_with_cache = CompletionUsage {
            prompt_tokens: 1000,
            completion_tokens: 200,
            total_tokens: 1200,
            prompt_tokens_details: Some(PromptTokensDetails {
                audio_tokens: None,
                cached_tokens: Some(800),
            }),
            completion_tokens_details: Some(CompletionTokensDetails::default()),
        };
        assert_eq!(
            extract_cache_hit_ratio(Some(&usage_with_cache)),
            Some(dec!(0.8))
        );

        let usage_no_details = CompletionUsage {
            prompt_tokens: 500,
            completion_tokens: 100,
            total_tokens: 600,
            prompt_tokens_details: None,
            completion_tokens_details: None,
        };
        assert_eq!(extract_cache_hit_ratio(Some(&usage_no_details)), None);

        assert_eq!(extract_cache_hit_ratio(None), None);
    }

    #[test]
    fn openai_narrator_constructs_without_network() {
        // Smoke test: just verify `new` takes the three inputs and yields
        // a narrator. No HTTP call, no env access.
        let _ = OpenAiNarrator::new(
            "https://api.deepseek.com/v1",
            "sk-test-key",
            "deepseek-chat",
        );
    }
}
