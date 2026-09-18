//! TS-01 — HTTP transport for the TypeSafe System One endpoint.
//!
//! Prod impl: [`HttpSystemOneClient`] posts to `/v1/systemone` with a bearer
//! credential. It is only constructed when the integration is enabled and a
//! credential exists, so this type never describes a disabled call.
//!
//! Test impl: [`MockSystemOneClient`] hands back a scripted response, so the
//! judgement services can be exercised without HTTP. The HTTP layer itself is
//! tested against a local mock server, not against the vendor.

// @anchor exchange:router:typesafe-client
// @tags api

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use rand::Rng;
use reqwest::header::{HeaderValue, RETRY_AFTER};
use reqwest::StatusCode;
use serde::Serialize;

use super::types::{
    backoff_delay, CallPolicy, Question, State, SystemOneRequest, SystemOneResponse,
    TypeSafeError,
};
use super::types::{Answer, MODEL_PINNED, Usage};

/// Performs one evaluation. Object-safe.
#[async_trait]
pub trait SystemOneClient: Send + Sync {
    /// `policy` is a per-call argument because the two call sites have
    /// opposite budgets: pre-trade runs inside a modal the user waits on,
    /// post-trade runs async where a retry harms nobody.
    async fn judge(
        &self,
        request: &SystemOneRequest,
        policy: CallPolicy,
    ) -> Result<SystemOneResponse, TypeSafeError>;

    /// The model id this client sends, for logs and the health probe.
    fn model(&self) -> &str;
}

/// Wire body. Private because callers must not choose the model: it is
/// injected from client configuration so it cannot be unpinned at a call
/// site.
#[derive(Debug, Serialize)]
struct WireRequest<'a> {
    state: &'a State,
    model: &'a str,
    questions: &'a BTreeMap<String, Question>,
}

// ─────────────────────────────────────────────────────────────────────────
// Production client
// ─────────────────────────────────────────────────────────────────────────

/// Posts evaluations to a TypeSafe-compatible endpoint.
///
/// The credential is held in memory for the process lifetime and is never
/// logged: [`HttpSystemOneClient`]'s `Debug` output redacts it.
pub struct HttpSystemOneClient {
    http: reqwest::Client,
    endpoint: String,
    api_key: String,
    model: String,
}

impl HttpSystemOneClient {
    /// `base_url` may carry a trailing slash. `model` should be a pinned id
    /// rather than an alias once a threshold is tuned against it.
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let base = base_url.into().trim_end_matches('/').to_string();
        Self {
            // Connection pooling is already on by default. Per-request
            // timeouts come from `CallPolicy`, so no client-level timeout is
            // set here: one would silently cap the post-trade path too.
            http: reqwest::Client::new(),
            endpoint: format!("{base}/v1/systemone"),
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    /// Sends with retries for transient failures, honouring the vendor's
    /// `retry-after` when present.
    ///
    /// The loop only exits by returning: the final attempt either succeeds or
    /// reports why it did not, so no attempt can be silently dropped.
    async fn attempt(
        &self,
        body: &serde_json::Value,
        policy: CallPolicy,
    ) -> Result<SystemOneResponse, TypeSafeError> {
        let max_attempts = policy.max_attempts.max(1);
        let mut failures: u32 = 0;

        loop {
            match self.send(body, policy.timeout).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    failures += 1;
                    if failures >= max_attempts || !err.is_retryable() {
                        return Err(err);
                    }
                    let delay = err
                        .retry_after_hint()
                        .map(|hint| hint.min(policy.max_delay))
                        .unwrap_or_else(|| {
                            let jitter: f64 = rand::thread_rng().gen();
                            backoff_delay(failures - 1, &policy, jitter)
                        });
                    tracing::debug!(
                        error = %err,
                        attempt = failures,
                        delay_ms = delay.as_millis(),
                        "typesafe: retrying after transient failure"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// One HTTP attempt. Classifies the response into a typed error.
    async fn send(
        &self,
        body: &serde_json::Value,
        timeout: Duration,
    ) -> Result<SystemOneResponse, TypeSafeError> {
        let response = self
            .http
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .timeout(timeout)
            .json(body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    TypeSafeError::Timeout
                } else {
                    TypeSafeError::Network(e.to_string())
                }
            })?;

        let status = response.status();
        if status.is_success() {
            return response
                .json::<SystemOneResponse>()
                .await
                .map_err(|e| TypeSafeError::Parse(format!("response shape mismatch: {e}")));
        }

        let retry_after = parse_retry_after(response.headers().get(RETRY_AFTER));
        match status {
            StatusCode::UNAUTHORIZED => Err(TypeSafeError::Unauthorized),
            StatusCode::TOO_MANY_REQUESTS => Err(TypeSafeError::RateLimit { retry_after }),
            // 529 is not a standard code, so it has no `StatusCode` constant.
            // It must be matched before the blanket 5xx arm below.
            status if status.as_u16() == 529 => Err(TypeSafeError::Overloaded { retry_after }),
            // An undocumented 5xx is treated as transient.
            status if status.is_server_error() => {
                Err(TypeSafeError::Network(format!("upstream {status}")))
            }
            status => {
                // 422 names the offending field. Truncated: the body goes in
                // a log line, and it is third-party text.
                let detail: String = response
                    .text()
                    .await
                    .unwrap_or_default()
                    .chars()
                    .take(300)
                    .collect();
                Err(TypeSafeError::BadRequest(format!("{status}: {detail}")))
            }
        }
    }

    /// The model id this client sends.
    pub fn model(&self) -> &str {
        &self.model
    }
}

impl std::fmt::Debug for HttpSystemOneClient {
    /// Redacts the credential. A `Debug` derive here would put a live key
    /// into any log line that formats the client.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpSystemOneClient")
            .field("endpoint", &self.endpoint)
            .field("api_key", &"<redacted>")
            .field("model", &self.model)
            .finish()
    }
}

#[async_trait]
impl SystemOneClient for HttpSystemOneClient {
    async fn judge(
        &self,
        request: &SystemOneRequest,
        policy: CallPolicy,
    ) -> Result<SystemOneResponse, TypeSafeError> {
        // Assert the caller's half of the contract before anything else: a
        // malformed question should cost a local error, not a round trip.
        request.validate()?;

        let body = serde_json::to_value(WireRequest {
            state: &request.state,
            model: &self.model,
            questions: &request.questions,
        })
        .map_err(|e| TypeSafeError::Parse(format!("request serialize failed: {e}")))?;

        let response = self.attempt(&body, policy).await?;

        // A response that omits an answer we asked for is a parse failure,
        // not a partial success. Callers must never infer a missing answer.
        if response.answers.len() != request.questions.len() {
            return Err(TypeSafeError::Parse(format!(
                "asked {} questions, received {} answers",
                request.questions.len(),
                response.answers.len()
            )));
        }

        tracing::debug!(
            model = %response.model,
            questions = request.questions.len(),
            input_tokens = response.usage.input_tokens,
            output_tokens = response.usage.output_tokens,
            "typesafe: evaluation complete"
        );

        Ok(response)
    }

    fn model(&self) -> &str {
        &self.model
    }
}

/// `retry-after` is documented in seconds by the vendor.
///
/// The HTTP-date form is not handled: parsing it wrongly would produce a
/// worse delay than falling back to our own backoff, and the seconds form is
/// the one the API sends.
fn parse_retry_after(value: Option<&HeaderValue>) -> Option<Duration> {
    let raw = value?.to_str().ok()?.trim();
    raw.parse::<u64>().ok().map(Duration::from_secs)
}

// ─────────────────────────────────────────────────────────────────────────
// Test double
// ─────────────────────────────────────────────────────────────────────────

/// Test double. Hand it one scripted result up front.
///
/// `judge` consumes it, so a second call panics: tests fail loudly if the
/// system under test issues an unexpected extra request.
pub struct MockSystemOneClient {
    response: Mutex<Option<Result<SystemOneResponse, TypeSafeError>>>,
}

impl MockSystemOneClient {
    pub fn new(response: Result<SystemOneResponse, TypeSafeError>) -> Self {
        Self {
            response: Mutex::new(Some(response)),
        }
    }

    /// A client that always succeeds with the given answers.
    pub fn answering(answers: impl IntoIterator<Item = (String, Answer)>) -> Self {
        Self::new(Ok(SystemOneResponse {
            model: MODEL_PINNED.to_string(),
            answers: answers.into_iter().collect(),
            usage: Usage::default(),
        }))
    }
}

#[async_trait]
impl SystemOneClient for MockSystemOneClient {
    async fn judge(
        &self,
        _request: &SystemOneRequest,
        _policy: CallPolicy,
    ) -> Result<SystemOneResponse, TypeSafeError> {
        self.response
            .lock()
            .expect("MockSystemOneClient mutex poisoned")
            .take()
            .expect("MockSystemOneClient::judge called more than once")
    }

    fn model(&self) -> &str {
        MODEL_PINNED
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;
    use crate::services::typesafe::types::{Usage, DEFAULT_BASE_URL};

    fn request(q: Question) -> SystemOneRequest {
        SystemOneRequest {
            state: json!({"typed_tag": "brkout", "known_tags": ["breakout"]}),
            questions: [("setup_tag".to_string(), q)].into_iter().collect(),
        }
    }

    fn choice_question() -> Question {
        Question::choice(
            "Which existing setup does this trade belong to?",
            [
                ("breakout".to_string(), None),
                ("mean_reversion".to_string(), None),
            ],
        )
    }

    fn ok_body(choice: &str) -> serde_json::Value {
        json!({
            "model": MODEL_PINNED,
            "answers": {
                "setup_tag": {
                    "type": "choice",
                    "choice": choice,
                    "probabilities": {"breakout": 0.85, "mean_reversion": 0.15},
                    "confidence": 0.82
                }
            },
            "usage": {"input_tokens": 120, "output_tokens": 8}
        })
    }

    /// Builds the mock server and client together. Returning both keeps the
    /// guard alive for the caller's assertions.
    async fn server_and_client() -> (mockito::ServerGuard, HttpSystemOneClient) {
        let server = mockito::Server::new_async().await;
        let client = HttpSystemOneClient::new(server.url(), "ts-test-key", MODEL_PINNED);
        (server, client)
    }

    #[tokio::test]
    async fn posts_bearer_credential_and_injects_the_configured_model() {
        let (mut server, client) = server_and_client().await;
        let mock = server
            .mock("POST", "/v1/systemone")
            .match_header("authorization", "Bearer ts-test-key")
            .match_body(mockito::Matcher::PartialJson(json!({"model": MODEL_PINNED})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ok_body("breakout").to_string())
            .expect(1)
            .create_async()
            .await;

        let response = client
            .judge(&request(choice_question()), CallPolicy::pre_trade())
            .await
            .expect("200 parses");

        mock.assert_async().await;
        assert_eq!(response.model, MODEL_PINNED);
        assert_eq!(response.usage, Usage { input_tokens: 120, output_tokens: 8 });
        match response.answers.get("setup_tag").expect("answer present") {
            Answer::Choice { choice, confidence, .. } => {
                assert_eq!(choice, "breakout");
                assert_eq!(*confidence, dec!(0.82));
            }
            other => panic!("expected Choice, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unauthorized_is_not_retried_even_on_the_post_trade_policy() {
        let (mut server, client) = server_and_client().await;
        // expect(1) is the assertion: a retried 401 would make this fail.
        let mock = server
            .mock("POST", "/v1/systemone")
            .with_status(401)
            .expect(1)
            .create_async()
            .await;

        let err = client
            .judge(&request(choice_question()), CallPolicy::post_trade())
            .await
            .expect_err("401 must error");

        mock.assert_async().await;
        assert_eq!(err, TypeSafeError::Unauthorized);
    }

    #[tokio::test]
    async fn pre_trade_policy_does_not_retry_a_rate_limit() {
        let (mut server, client) = server_and_client().await;
        // D4: a retry-after of one second cannot fit a 500 ms budget.
        let mock = server
            .mock("POST", "/v1/systemone")
            .with_status(429)
            .with_header("retry-after", "1")
            .expect(1)
            .create_async()
            .await;

        let err = client
            .judge(&request(choice_question()), CallPolicy::pre_trade())
            .await
            .expect_err("429 must error");

        mock.assert_async().await;
        assert_eq!(
            err,
            TypeSafeError::RateLimit { retry_after: Some(Duration::from_secs(1)) }
        );
    }

    #[tokio::test]
    async fn post_trade_policy_exhausts_attempts_on_a_persistent_overload() {
        let (mut server, client) = server_and_client().await;
        let policy = CallPolicy {
            timeout: Duration::from_secs(2),
            max_attempts: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        };
        let mock = server
            .mock("POST", "/v1/systemone")
            .with_status(529)
            .expect(2)
            .create_async()
            .await;

        let err = client
            .judge(&request(choice_question()), policy)
            .await
            .expect_err("529 must error");

        mock.assert_async().await;
        assert!(matches!(err, TypeSafeError::Overloaded { .. }));
    }

    #[tokio::test]
    async fn post_trade_policy_recovers_when_a_retry_succeeds() {
        let (mut server, client) = server_and_client().await;
        let policy = CallPolicy {
            timeout: Duration::from_secs(2),
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        };
        // First call fails, the retry is served by the second mock. mockito
        // stops serving a mock once its expectation is exhausted.
        let fail = server
            .mock("POST", "/v1/systemone")
            .with_status(529)
            .expect(1)
            .create_async()
            .await;
        let succeed = server
            .mock("POST", "/v1/systemone")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(ok_body("breakout").to_string())
            .expect(1)
            .create_async()
            .await;

        let response = client
            .judge(&request(choice_question()), policy)
            .await
            .expect("the retry must recover");

        fail.assert_async().await;
        succeed.assert_async().await;
        assert_eq!(response.answers.len(), 1);
    }

    #[tokio::test]
    async fn unprocessable_entity_surfaces_the_provider_detail() {
        let (mut server, client) = server_and_client().await;
        let mock = server
            .mock("POST", "/v1/systemone")
            .with_status(422)
            .with_body(r#"{"detail":"criteria: must not be empty"}"#)
            .expect(1)
            .create_async()
            .await;

        let err = client
            .judge(&request(choice_question()), CallPolicy::pre_trade())
            .await
            .expect_err("422 must error");

        mock.assert_async().await;
        match err {
            TypeSafeError::BadRequest(detail) => {
                assert!(detail.contains("422"));
                assert!(detail.contains("must not be empty"));
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_missing_answer_is_a_parse_failure_not_a_partial_success() {
        let (mut server, client) = server_and_client().await;
        let mock = server
            .mock("POST", "/v1/systemone")
            .with_status(200)
            .with_header("content-type", "application/json")
            // Two questions asked, one answered.
            .with_body(json!({"model": MODEL_PINNED, "answers": {}}).to_string())
            .expect(1)
            .create_async()
            .await;

        let requested = SystemOneRequest {
            state: json!({"typed_tag": "brkout"}),
            questions: [
                ("setup_tag".to_string(), choice_question()),
                ("confidence_check".to_string(), Question::noul("Is this clear?")),
            ]
            .into_iter()
            .collect(),
        };

        let err = client
            .judge(&requested, CallPolicy::pre_trade())
            .await
            .expect_err("mismatched answer count must error");

        mock.assert_async().await;
        match err {
            TypeSafeError::Parse(msg) => assert!(msg.contains("asked 2 questions")),
            other => panic!("expected Parse, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn malformed_questions_are_rejected_before_the_wire() {
        let (mut server, client) = server_and_client().await;
        // No mock registered: any request at all is a test failure.
        let _guard = server
            .mock("POST", "/v1/systemone")
            .expect(0)
            .create_async()
            .await;

        let err = client
            .judge(
                &request(Question::score("rate the exit", Vec::new())),
                CallPolicy::pre_trade(),
            )
            .await
            .expect_err("an invalid question must not be sent");

        assert!(matches!(err, TypeSafeError::BadRequest(_)));
    }

    #[test]
    fn debug_output_redacts_the_credential() {
        let client = HttpSystemOneClient::new(
            DEFAULT_BASE_URL,
            "ts-super-secret",
            MODEL_PINNED,
        );
        let rendered = format!("{client:?}");
        assert!(!rendered.contains("ts-super-secret"), "key leaked into Debug");
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains(MODEL_PINNED));
    }

    #[test]
    fn base_url_trailing_slash_does_not_double_up() {
        let client = HttpSystemOneClient::new("https://api.typesafe.ai/", "k", MODEL_PINNED);
        assert_eq!(client.endpoint, "https://api.typesafe.ai/v1/systemone");
    }

    #[test]
    fn retry_after_parses_seconds_and_ignores_other_forms() {
        assert_eq!(
            parse_retry_after(Some(&HeaderValue::from_static("12"))),
            Some(Duration::from_secs(12))
        );
        assert_eq!(
            parse_retry_after(Some(&HeaderValue::from_static(" 3 "))),
            Some(Duration::from_secs(3))
        );
        assert_eq!(parse_retry_after(Some(&HeaderValue::from_static("soon"))), None);
        assert_eq!(
            parse_retry_after(Some(&HeaderValue::from_static("Wed, 21 Oct 2026 07:28:00 GMT"))),
            None
        );
        assert_eq!(parse_retry_after(None), None);
    }

    #[tokio::test]
    async fn mock_client_hands_back_the_scripted_response() {
        // Guards the test double the service-level tests lean on.
        let client = MockSystemOneClient::answering([(
            "setup_tag".to_string(),
            Answer::Choice {
                choice: "breakout".to_string(),
                probabilities: [("breakout".to_string(), dec!(0.9))].into_iter().collect(),
                confidence: dec!(0.9),
            },
        )]);
        let response = client
            .judge(&request(choice_question()), CallPolicy::pre_trade())
            .await
            .expect("scripted ok");
        assert_eq!(response.answers.len(), 1);
    }
}
