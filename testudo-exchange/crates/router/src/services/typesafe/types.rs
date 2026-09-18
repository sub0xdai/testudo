//! TS-01 — TypeSafe System One (Jev) wire types.
//!
//! Mirrors `POST /v1/systemone`: one `state`, a map of typed `questions`,
//! one typed `Answer` per question id. Question ids are ours and are never
//! sent to the model, so each question carries its full meaning in
//! `instructions`.
//!
//! Design and scope: `docs/plans/typesafe-jev-sniper-integration.md`.

// @anchor exchange:router:typesafe-types
// @tags api

use std::collections::BTreeMap;
use std::time::Duration;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Pinned model id.
///
/// Prefer this over [`MODEL_ALIAS`] once a confidence threshold is tuned
/// against real trades: an alias moves without a change on our side, so the
/// answers behind it can shift under a fixed threshold.
pub const MODEL_PINNED: &str = "jev-1.13.0";

/// Floating alias tracking the newest official release.
pub const MODEL_ALIAS: &str = "jev-latest";

/// Evaluation endpoint host, no trailing slash.
pub const DEFAULT_BASE_URL: &str = "https://api.typesafe.ai";

/// Environment variable holding the bearer credential.
///
/// Read in `main.rs` and never in the extension. A key shipped to a client is
/// shared with every user and every page the client runs on.
pub const API_KEY_ENV: &str = "TYPESAFE_API_KEY";

/// Option to rubric. `None` marks an option that stands on its own.
///
/// A `BTreeMap`, not a `HashMap`, so option order is stable and one request
/// serialises identically on every attempt.
pub type ChoiceCriteria = BTreeMap<String, Option<String>>;

/// Ordered level descriptions, lowest to highest. Minimum two.
pub type ScoreCriteria = Vec<String>;

/// The material one request evaluates.
///
/// A bare `Value` is deliberate. Callers build it from their own typed
/// structs, so the typing lives where we own the data, and this slot stays a
/// passthrough for the vendor's three shapes. [`SystemOneRequest::validate`]
/// rejects anything the vendor would reject with a 422.
pub type State = serde_json::Value;

/// What a yes and a no mean, for a question where both deserve a rubric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NoulCriteria {
    /// Meaning of a value near 1.
    #[serde(rename = "true")]
    pub yes: String,
    /// Meaning of a value near 0.
    #[serde(rename = "false")]
    pub no: String,
}

/// One typed judgement. The `type` tag is the wire discriminator.
///
/// `instructions` is always a plain string here. The vendor also accepts
/// structured instructions; we do not send them yet.
// ponytail: text-only instructions; widen to the object/array form when a
// question genuinely cannot be stated in prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Question {
    /// Probability that a condition holds. No separate confidence is
    /// returned: a value near 0.5 means an even split, not medium intensity.
    Noul {
        instructions: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// One option out of a closed set.
    ///
    /// The model cannot pick an option that was not offered, so callers must
    /// offer every option an answer may legitimately be, including a
    /// no-match outcome.
    Choice {
        instructions: String,
        criteria: ChoiceCriteria,
    },
    /// Probability-weighted position across ordered levels.
    Score {
        instructions: String,
        criteria: ScoreCriteria,
    },
}

impl Question {
    /// A yes/no question with no rubric.
    pub fn noul(instructions: impl Into<String>) -> Self {
        Self::Noul {
            instructions: instructions.into(),
            criteria: None,
        }
    }

    /// A yes/no question where both outcomes are worth describing.
    pub fn noul_with_criteria(
        instructions: impl Into<String>,
        yes: impl Into<String>,
        no: impl Into<String>,
    ) -> Self {
        Self::Noul {
            instructions: instructions.into(),
            criteria: Some(NoulCriteria {
                yes: yes.into(),
                no: no.into(),
            }),
        }
    }

    /// One option out of a closed set.
    pub fn choice(
        instructions: impl Into<String>,
        options: impl IntoIterator<Item = (String, Option<String>)>,
    ) -> Self {
        Self::Choice {
            instructions: instructions.into(),
            criteria: options.into_iter().collect(),
        }
    }

    /// A rating across ordered levels.
    pub fn score(
        instructions: impl Into<String>,
        levels: impl IntoIterator<Item = String>,
    ) -> Self {
        Self::Score {
            instructions: instructions.into(),
            criteria: levels.into_iter().collect(),
        }
    }

    /// Primitive name, for logs and metrics labels.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
        }
    }
}

/// A full evaluation request.
///
/// The `model` field is absent on purpose: the client injects it from its own
/// configuration, so a call site cannot silently unpin the model.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SystemOneRequest {
    pub state: State,
    pub questions: BTreeMap<String, Question>,
}

impl SystemOneRequest {
    /// Checks the invariants the vendor rejects with a bare 422.
    ///
    /// Called once per request before any network work: a malformed question
    /// should cost a local error, not a round trip.
    pub fn validate(&self) -> Result<(), TypeSafeError> {
        if self.questions.is_empty() {
            return Err(TypeSafeError::BadRequest("request has no questions".into()));
        }
        match &self.state {
            State::String(_) | State::Object(_) | State::Array(_) => {}
            _ => {
                return Err(TypeSafeError::BadRequest(
                    "state must be a string, object, or array of text values".into(),
                ));
            }
        }
        for (id, question) in &self.questions {
            match question {
                Question::Noul { .. } => {}
                Question::Choice { criteria, .. } if criteria.is_empty() => {
                    return Err(TypeSafeError::BadRequest(format!(
                        "choice {id} offers no options"
                    )));
                }
                Question::Choice { .. } => {}
                Question::Score { criteria, .. } if criteria.len() < 2 => {
                    return Err(TypeSafeError::BadRequest(format!(
                        "score {id} needs at least two levels"
                    )));
                }
                Question::Score { .. } => {}
            }
        }
        Ok(())
    }
}

/// Token accounting. Output tokens are free, but both are logged: they are
/// the only lever on cost per trade.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
}

/// One answer per question id.
///
/// Choice and Score carry `confidence`, which summarises how concentrated the
/// distribution is. It is not a statement about whether the answer is right,
/// and it is not permission to act.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Answer {
    /// Probability that the answer is yes, in `[0,1]`.
    Noul { noul: Decimal },
    /// The highest-probability option and the full distribution.
    Choice {
        choice: String,
        probabilities: BTreeMap<String, Decimal>,
        confidence: Decimal,
    },
    /// The probability-weighted position across the levels.
    Score {
        score: Decimal,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, Decimal>,
        confidence: Decimal,
    },
}

impl Answer {
    /// Primitive name, for logs and for reporting a mismatch between the
    /// question we asked and the answer we got back.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Noul { .. } => "noul",
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
        }
    }
}

/// A successful evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemOneResponse {
    /// The versioned model id that answered, which is not necessarily the
    /// name we sent. Logged so a result can be traced to a model build.
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    /// Defaulted rather than required: losing a judgement because the vendor
    /// omitted a usage block would be the wrong trade.
    #[serde(default)]
    pub usage: Usage,
}

/// Per-call transport policy.
///
/// The two call sites have opposite budgets, so this is a per-call argument
/// rather than client state:
///
/// - pre-trade runs inside a modal a user is waiting on. A `retry-after` of
///   even one second blows the budget, so it never retries.
/// - post-trade runs async, where a retry blocks nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallPolicy {
    /// Per HTTP attempt.
    pub timeout: Duration,
    /// Total attempts including the first. `1` disables retry.
    pub max_attempts: u32,
    /// Base for exponential backoff between attempts.
    pub base_delay: Duration,
    /// Sleep cap, so a hostile `retry-after` cannot park a worker.
    pub max_delay: Duration,
}

impl CallPolicy {
    /// UC-1. Never retries: see the type docs.
    pub const fn pre_trade() -> Self {
        Self {
            timeout: Duration::from_millis(500),
            max_attempts: 1,
            base_delay: Duration::ZERO,
            max_delay: Duration::ZERO,
        }
    }

    /// UC-3. Retries transient failures with jittered backoff.
    pub const fn post_trade() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_attempts: 4,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(8),
        }
    }
}

/// Exponential backoff with full jitter, capped at `policy.max_delay`.
///
/// `jitter` is a caller-supplied value in `[0,1)`. Taking it as an argument
/// instead of reading an RNG makes this a pure function, so the schedule is
/// testable without seeding a global generator. `attempt` is zero-based: `0`
/// is the delay before the second attempt.
pub fn backoff_delay(attempt: u32, policy: &CallPolicy, jitter: f64) -> Duration {
    let jitter = if jitter.is_finite() { jitter.clamp(0.0, 1.0) } else { 0.0 };
    let growth = policy.base_delay.as_secs_f64() * 2f64.powi(attempt.min(16) as i32);
    let window = growth.min(policy.max_delay.as_secs_f64()).max(0.0);
    Duration::from_secs_f64(window * jitter)
}

/// Transport and service failures.
///
/// There is deliberately no `Disabled` variant. A disabled integration is
/// modelled as no client at all (an `Option<Arc<dyn SystemOneClient>>` field
/// on `AppState`), so this type only ever describes a call that was actually
/// attempted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TypeSafeError {
    #[error("typesafe request timed out")]
    Timeout,
    /// HTTP 429.
    #[error("typesafe rate limited (retry-after {retry_after:?})")]
    RateLimit { retry_after: Option<Duration> },
    /// HTTP 529, the vendor's overload signal.
    #[error("typesafe overloaded (retry-after {retry_after:?})")]
    Overloaded { retry_after: Option<Duration> },
    /// HTTP 401. Never retried: the credential will not fix itself, and
    /// retrying turns one bad key into a request storm.
    #[error("typesafe credential rejected")]
    Unauthorized,
    /// HTTP 422 or a local invariant failure.
    #[error("typesafe request rejected: {0}")]
    BadRequest(String),
    /// The response body did not match the documented shape.
    #[error("typesafe response failed to parse: {0}")]
    Parse(String),
    /// Connection failure, or a 5xx the vendor does not document.
    #[error("typesafe network error: {0}")]
    Network(String),
}

impl TypeSafeError {
    /// Transient enough to be worth another attempt.
    ///
    /// A timeout counts: an evaluation has no side effects, so a retry cannot
    /// duplicate work. A 401 and a 422 do not, and are never retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::RateLimit { .. } | Self::Overloaded { .. } | Self::Network(_)
        )
    }

    /// The vendor's own backoff hint, when the response carried one.
    pub fn retry_after_hint(&self) -> Option<Duration> {
        match self {
            Self::RateLimit { retry_after } | Self::Overloaded { retry_after } => *retry_after,
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;

    fn question_map(
        pairs: impl IntoIterator<Item = (&'static str, Question)>,
    ) -> BTreeMap<String, Question> {
        pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[test]
    fn noul_serialises_with_type_tag_and_omits_absent_criteria() {
        let q = Question::noul("Does the note describe a rule break?");
        let value = serde_json::to_value(&q).expect("noul serialises");
        assert_eq!(value["type"], "noul");
        assert_eq!(value["instructions"], "Does the note describe a rule break?");
        assert!(
            value.get("criteria").is_none(),
            "absent criteria must not be sent as null"
        );
    }

    #[test]
    fn noul_criteria_uses_true_and_false_keys() {
        let q = Question::noul_with_criteria(
            "Planned exit?",
            "Written down before entry",
            "Improvised",
        );
        let value = serde_json::to_value(&q).expect("noul serialises");
        assert_eq!(value["criteria"]["true"], "Written down before entry");
        assert_eq!(value["criteria"]["false"], "Improvised");
    }

    #[test]
    fn choice_serialises_options_as_an_object_with_null_rubrics() {
        let q = Question::choice(
            "Which existing setup does this trade belong to?",
            [
                ("breakout".to_string(), Some("Range break on volume".to_string())),
                ("mean_reversion".to_string(), None),
            ],
        );
        let value = serde_json::to_value(&q).expect("choice serialises");
        assert_eq!(value["type"], "choice");
        assert_eq!(value["criteria"]["breakout"], "Range break on volume");
        assert!(value["criteria"]["mean_reversion"].is_null());
    }

    #[test]
    fn choice_option_order_is_stable_across_serialisation() {
        let options = || {
            [
                ("zulu".to_string(), None),
                ("alpha".to_string(), None),
                ("mike".to_string(), None),
            ]
        };
        let first = serde_json::to_string(&Question::choice("pick", options())).unwrap();
        let second = serde_json::to_string(&Question::choice("pick", options())).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn validate_rejects_empty_question_map() {
        let request = SystemOneRequest {
            state: json!({"symbol": "BTCUSDT"}),
            questions: BTreeMap::new(),
        };
        match request.validate() {
            Err(TypeSafeError::BadRequest(msg)) => assert!(msg.contains("no questions")),
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_scalar_state() {
        for state in [json!(42), json!(true), json!(null)] {
            let request = SystemOneRequest {
                state,
                questions: question_map([("q", Question::noul("hmm?"))]),
            };
            assert!(
                matches!(request.validate(), Err(TypeSafeError::BadRequest(_))),
                "scalar state must be rejected before the wire"
            );
        }
    }

    #[test]
    fn validate_accepts_all_three_documented_state_shapes() {
        for state in [json!("plain text"), json!({"a": 1}), json!(["one", "two"])] {
            let request = SystemOneRequest {
                state,
                questions: question_map([("q", Question::noul("hmm?"))]),
            };
            assert!(request.validate().is_ok(), "state shape must be accepted");
        }
    }

    #[test]
    fn validate_rejects_choice_with_no_options_and_score_with_one_level() {
        let no_options = SystemOneRequest {
            state: json!("x"),
            questions: question_map([("q", Question::choice("pick", Vec::new()))]),
        };
        assert!(matches!(
            no_options.validate(),
            Err(TypeSafeError::BadRequest(_))
        ));

        let one_level = SystemOneRequest {
            state: json!("x"),
            questions: question_map([("q", Question::score("rate", ["only".to_string()]))]),
        };
        match one_level.validate() {
            Err(TypeSafeError::BadRequest(msg)) => assert!(msg.contains("two levels")),
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn choice_answer_parses_distribution_and_confidence() {
        let raw = json!({
            "type": "choice",
            "choice": "breakout",
            "probabilities": {"breakout": 0.85, "mean_reversion": 0.10, "new_setup": 0.05},
            "confidence": 0.82
        });
        let answer: Answer = serde_json::from_value(raw).expect("choice answer parses");
        match answer {
            Answer::Choice { choice, probabilities, confidence } => {
                assert_eq!(choice, "breakout");
                assert_eq!(probabilities.get("breakout"), Some(&dec!(0.85)));
                assert_eq!(confidence, dec!(0.82));
            }
            other => panic!("expected Choice, got {other:?}"),
        }
    }

    #[test]
    fn score_answer_parses_weighted_value_and_legend() {
        let raw = json!({
            "type": "score",
            "score": 1.6,
            "legend": {"0": "Planned", "1": "Partly planned", "2": "Improvised"},
            "probabilities": {"0": 0.05, "1": 0.30, "2": 0.65},
            "confidence": 0.78
        });
        let answer: Answer = serde_json::from_value(raw).expect("score answer parses");
        match answer {
            Answer::Score { score, legend, .. } => {
                assert_eq!(score, dec!(1.6));
                assert_eq!(legend.get("2").map(String::as_str), Some("Improvised"));
            }
            other => panic!("expected Score, got {other:?}"),
        }
    }

    #[test]
    fn response_survives_a_missing_usage_block() {
        let raw = json!({
            "model": "jev-1.13.0",
            "answers": {"q": {"type": "noul", "noul": 0.92}}
        });
        let response: SystemOneResponse =
            serde_json::from_value(raw).expect("usage is optional on our side");
        assert_eq!(response.model, "jev-1.13.0");
        assert_eq!(response.usage, Usage::default());
    }

    #[test]
    fn unknown_answer_type_is_a_parse_failure_not_a_silent_pass() {
        let raw = json!({"model": "jev-1.13.0", "answers": {"q": {"type": "vibe", "noul": 0.5}}});
        assert!(serde_json::from_value::<SystemOneResponse>(raw).is_err());
    }

    #[test]
    fn retryable_classification_matches_the_documented_guidance() {
        assert!(TypeSafeError::Timeout.is_retryable());
        assert!(TypeSafeError::RateLimit { retry_after: None }.is_retryable());
        assert!(TypeSafeError::Overloaded { retry_after: None }.is_retryable());
        assert!(TypeSafeError::Network("connection reset".into()).is_retryable());
        assert!(!TypeSafeError::Unauthorized.is_retryable());
        assert!(!TypeSafeError::BadRequest("bad field".into()).is_retryable());
        assert!(!TypeSafeError::Parse("bad shape".into()).is_retryable());
    }

    #[test]
    fn only_rate_limit_and_overload_carry_a_retry_after_hint() {
        let hint = Duration::from_secs(3);
        assert_eq!(
            TypeSafeError::RateLimit { retry_after: Some(hint) }.retry_after_hint(),
            Some(hint)
        );
        assert_eq!(
            TypeSafeError::Overloaded { retry_after: Some(hint) }.retry_after_hint(),
            Some(hint)
        );
        assert_eq!(TypeSafeError::Timeout.retry_after_hint(), None);
    }

    #[test]
    fn pre_trade_policy_never_retries() {
        // The whole point of the split policy: one attempt, inside the modal.
        assert_eq!(CallPolicy::pre_trade().max_attempts, 1);
        assert!(CallPolicy::post_trade().max_attempts > 1);
    }

    #[test]
    fn backoff_grows_with_attempts_and_respects_the_cap() {
        let policy = CallPolicy::post_trade();
        // jitter = 1.0 selects the full window, exposing the raw schedule.
        let first = backoff_delay(0, &policy, 1.0);
        let second = backoff_delay(1, &policy, 1.0);
        assert!(second > first, "attempts must widen the window");
        assert_eq!(first, policy.base_delay);

        // 250ms * 2^16 would be ~4.5 hours without the cap.
        assert_eq!(backoff_delay(16, &policy, 1.0), policy.max_delay);
    }

    #[test]
    fn backoff_is_jittered_monotonically_and_survives_bad_input() {
        let policy = CallPolicy::post_trade();
        assert_eq!(backoff_delay(3, &policy, 0.0), Duration::ZERO);
        assert!(
            backoff_delay(3, &policy, 0.5) < backoff_delay(3, &policy, 1.0),
            "jitter must scale the window"
        );
        // NaN would panic inside f64::clamp; it must fall back instead.
        assert_eq!(backoff_delay(3, &policy, f64::NAN), Duration::ZERO);
        assert_eq!(backoff_delay(3, &policy, 99.0), backoff_delay(3, &policy, 1.0));
        assert_eq!(backoff_delay(3, &policy, -5.0), Duration::ZERO);
    }

    #[test]
    fn pre_trade_policy_has_a_zero_window_so_it_cannot_sleep() {
        let policy = CallPolicy::pre_trade();
        assert_eq!(backoff_delay(0, &policy, 1.0), Duration::ZERO);
        assert_eq!(backoff_delay(9, &policy, 1.0), Duration::ZERO);
    }
}
