//! TS-01 — UC-3: journal note attribution.
//!
//! After a trader writes a note against a closed trade, one batched request
//! reads it three ways: which of their own setup tags it describes, whether it
//! admits to a rule break, and whether the exit was planned. The answers land
//! as typed columns on `journal_trades`, and the raw vendor reply lands in
//! `trade_events` as an append-only audit row.
//!
//! # Why this shape
//!
//! Dignitas already scores note *presence*. Nothing read note *content*, so a
//! trader's own written self-report contributed nothing to their analytics.
//! These three judgements are the smallest set that turns prose into fields the
//! coach's numeric detectors and a tag-keyed calibration lookup can use.
//!
//! # Thresholds
//!
//! Only UC-1 thresholds a judgement, because a wrong tag there corrupts a
//! calibration lookup. Here the raw probabilities are stored unthresholded and
//! every consumer picks its own cutoff. Thresholding at write time would bake
//! one policy into the data and make it unrecoverable without re-running
//! inference.
//!
//! Design and scope: `docs/plans/typesafe-jev-sniper-integration.md`.

// @anchor exchange:router:typesafe-attribution
// @tags api

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use super::client::SystemOneClient;
use super::service::{tag_options, without_sentinel_collisions, TagCandidate};
use super::types::{
    Answer, CallPolicy, Question, SystemOneRequest, SystemOneResponse, TypeSafeError, Usage,
};
use crate::services::journal_service::fetch_setup_tags;

/// Option set size for the tag Choice. Matches the vocabulary cap so UC-1 and
/// UC-3 offer the same tags for the same trader.
const TAG_LIMIT: i64 = 40;

/// Question id: which of the trader's tags does the note describe.
pub const TAG_QUESTION_ID: &str = "note_setup_tag";

/// Question id: does the note admit to a rule break.
pub const RULE_BREAK_QUESTION_ID: &str = "rule_break";

/// Question id: was the exit planned.
pub const EXIT_DISCIPLINE_QUESTION_ID: &str = "exit_discipline";

/// Exit-discipline levels, lowest to highest.
///
/// Each level describes a concrete situation rather than an intensity: the
/// vendor's guidance is that score levels must stand on their own so the model
/// has something specific to match against.
pub const EXIT_DISCIPLINE_LEVELS: [&str; 3] = [
    "The exit was decided before entry, or the note says the pre-decided plan was honoured",
    "The exit was decided while managing the trade, but was weighed against the original plan",
    "The exit was improvised in reaction to price, with no prior plan to compare against",
];

const TAG_INSTRUCTIONS: &str = "\
`note` is what a trader wrote about the trade in `trade` after it closed. \
`existing_tags` lists setup tags already used on their closed trades, each with \
the number of closed trades carrying it. `existing_tags` is the only source of \
tags: do not invent one. Which single option names the setup the note is about? \
Judge the setup the trade was taken on, not the outcome and not the exit. If the \
note does not say enough about the setup to place it, choose the option described \
as a new setup; do not guess at the closest-sounding tag.";

const RULE_BREAK_INSTRUCTIONS: &str = "\
Does the note in `note` describe the trader breaking a rule they hold, or making \
an execution mistake they are aware of? The admission has to be in the note: \
disappointment about a loss is not a rule break, and an unplanned exit is only a \
rule break if the note presents it as one. Trade `trade` is context for reading the \
note, not a source of rules.";

const EXIT_DISCIPLINE_INSTRUCTIONS: &str = "\
Reading `note`, how was the exit decided? Use `trade.close_reason`, \
`trade.duration_minutes`, and `trade.r_multiple` as context for interpreting the \
note, but judge what the note says about the decision, not how the trade \
performed. A profitable trade can have an improvised exit; a loss can have a \
planned one.";

/// Rubric for the no-match option in the tag Choice.
///
/// Kept beside the instructions rather than inline: the model reads it as an
/// option description, so it is part of the question's wording.
const NOVEL_TAG_RUBRIC: &str = "The note does not place this trade on a listed setup.";

// ─────────────────────────────────────────────────────────────────────────
// Stored shape
// ─────────────────────────────────────────────────────────────────────────

/// Discriminant for the `judgment_*` columns, mirrored by a CHECK constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributionStatus {
    /// Nothing was attempted. The state of every row written before this
    /// feature existed, and of rows whose save lost its spawned task.
    NotAttempted,
    /// The trade had no note. Not a failure: a closed trade with no reflection
    /// has nothing to read, and calling the model would burn a request to
    /// learn that.
    NoNote,
    /// A judgement is stored.
    Attributed,
    /// A judgement was attempted and produced nothing usable.
    Failed,
}

impl AttributionStatus {
    /// The value written to the `judgment_status` column.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::NoNote => "no_note",
            Self::Attributed => "attributed",
            Self::Failed => "failed",
        }
    }
}

/// The attribution as persisted to `journal_trades`.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteAttribution {
    pub model: String,
    /// The tag a listed setup matched, or `None` when the trader's vocabulary
    /// did not cover this note. Absence is a real answer here, not a gap.
    pub setup_tag: Option<String>,
    pub setup_confidence: Decimal,
    /// Raw probability that the note admits to a rule break.
    pub rule_break_noul: Decimal,
    /// Probability-weighted exit discipline across [`EXIT_DISCIPLINE_LEVELS`].
    pub exit_discipline: Decimal,
}

/// The vendor's own reply, kept verbatim for the audit row.
///
/// Separate from [`NoteAttribution`] on purpose: the columns are a projection
/// that later code may reinterpret, while the audit has to stay faithful to
/// what was actually said. Persisting only the projection would make a
/// threshold change unreconstructable.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AttributionAudit {
    pub model: String,
    pub usage: Usage,
    pub answers: BTreeMap<String, Answer>,
}

/// Outcome of asking the model about one note.
#[derive(Debug, Clone, PartialEq)]
pub enum Attribution {
    /// A judgement was produced. The stored projection and the audit travel
    /// together so they can be written in one transaction.
    Attributed {
        stored: NoteAttribution,
        audit: AttributionAudit,
    },
    /// The note was blank, so nothing was asked.
    NoNote,
    /// The call or its answer failed. Recorded so service failures stay
    /// distinguishable from "never ran".
    Failed(TypeSafeError),
}

// ─────────────────────────────────────────────────────────────────────────
// Inputs
// ─────────────────────────────────────────────────────────────────────────

/// The trade context one note judgement reads back from the row.
///
/// `notes` is loaded through `COALESCE(notes, '')` so emptiness is a `String`
/// invariant rather than a second nullability to reason about downstream.
#[derive(Debug, Clone, FromRow)]
pub struct NoteContext {
    pub id: Uuid,
    pub user_id: Uuid,
    pub trade_group_id: Option<Uuid>,
    pub symbol: String,
    pub side: String,
    pub notes: String,
    pub setup_tag: Option<String>,
    pub close_reason: Option<String>,
    pub duration_secs: i32,
    pub r_multiple: Option<Decimal>,
}

/// Loads the note and its context. `None` when the trade does not exist or is
/// not the caller's.
pub async fn load_note_context(
    pool: &PgPool,
    trade_id: Uuid,
    user_id: Uuid,
) -> Result<Option<NoteContext>, sqlx::Error> {
    sqlx::query_as::<_, NoteContext>(
        "SELECT id, user_id, trade_group_id, symbol, side, \
                COALESCE(notes, '') AS notes, setup_tag, close_reason, \
                duration_secs, r_multiple \
         FROM journal_trades \
         WHERE id = $1 AND user_id = $2",
    )
    .bind(trade_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// The trader's own tag vocabulary, as options the model may choose from.
pub async fn load_tag_candidates(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<TagCandidate>, sqlx::Error> {
    let entries = fetch_setup_tags(pool, user_id, TAG_LIMIT).await?;
    Ok(entries
        .into_iter()
        .map(|entry| TagCandidate {
            tag: entry.name,
            // COUNT(*) is i64. A negative or oversized value would be a
            // database fault, so clamp rather than wrap.
            uses: u32::try_from(entry.uses).unwrap_or(u32::MAX),
        })
        .collect())
}

// ─────────────────────────────────────────────────────────────────────────
// Request and interpretation
// ─────────────────────────────────────────────────────────────────────────

/// Builds the batched request. Separated from the call so the request can be
/// asserted without a client or a database.
///
/// One request, three questions, evaluated in parallel against the same state.
/// Output tokens are free and questions do not serialise, so batching costs
/// only the extra question tokens and saves two round trips.
pub fn build_attribution_request(
    context: &NoteContext,
    candidates: &[TagCandidate],
) -> SystemOneRequest {
    let offered = without_sentinel_collisions(candidates);

    let state = json!({
        "note": context.notes.trim(),
        "trade": {
            "symbol": context.symbol,
            "side": context.side,
            "setup_tag": context.setup_tag,
            "close_reason": context.close_reason,
            "duration_minutes": context.duration_secs / 60,
            "r_multiple": context.r_multiple,
        },
        "existing_tags": offered
            .iter()
            .map(|c| json!({ "tag": c.tag, "closed_trades": c.uses }))
            .collect::<Vec<_>>(),
    });

    let questions: BTreeMap<String, Question> = [
        (
            TAG_QUESTION_ID.to_string(),
            Question::choice(
                TAG_INSTRUCTIONS,
                tag_options(offered.iter().copied(), NOVEL_TAG_RUBRIC),
            ),
        ),
        (
            RULE_BREAK_QUESTION_ID.to_string(),
            Question::noul_with_criteria(
                RULE_BREAK_INSTRUCTIONS,
                "The note states or implies the trader broke a rule they hold, or made an \
                 execution mistake they are aware of",
                "The note records what happened without presenting a broken rule or an \
                 execution error",
            ),
        ),
        (
            EXIT_DISCIPLINE_QUESTION_ID.to_string(),
            Question::score(
                EXIT_DISCIPLINE_INSTRUCTIONS,
                EXIT_DISCIPLINE_LEVELS.iter().map(|s| s.to_string()),
            ),
        ),
    ]
    .into_iter()
    .collect();

    SystemOneRequest { state, questions }
}

/// Turns a response into the stored projection plus its audit.
///
/// Every question is required. A partial reply is a parse failure rather than a
/// partial attribution: a note judged on two of three axes is not the same
/// record as one judged on all three, and blending them would make the columns
/// unreadable.
fn interpret_attribution(
    response: SystemOneResponse,
    candidates: &[TagCandidate],
) -> Result<Attribution, TypeSafeError> {
    let tag_answer = response
        .answers
        .get(TAG_QUESTION_ID)
        .ok_or_else(|| TypeSafeError::Parse(format!("no answer for {TAG_QUESTION_ID}")))?;
    let Answer::Choice { choice, confidence, .. } = tag_answer else {
        return Err(TypeSafeError::Parse(format!(
            "{TAG_QUESTION_ID} answered with {} instead of a choice",
            tag_answer.kind()
        )));
    };

    // An unoffered name is discarded rather than written into the journal as a
    // tag, on the same reasoning as UC-1.
    let setup_tag = if choice == super::service::NOVEL_TAG_OPTION {
        None
    } else if candidates.iter().any(|c| &c.tag == choice) {
        Some(choice.clone())
    } else {
        return Err(TypeSafeError::Parse(format!("unoffered tag {choice}")));
    };

    let rule_break_noul = match response.answers.get(RULE_BREAK_QUESTION_ID) {
        Some(Answer::Noul { noul }) => *noul,
        Some(other) => {
            return Err(TypeSafeError::Parse(format!(
                "{RULE_BREAK_QUESTION_ID} answered with {} instead of a noul",
                other.kind()
            )))
        }
        None => {
            return Err(TypeSafeError::Parse(format!(
                "no answer for {RULE_BREAK_QUESTION_ID}"
            )))
        }
    };

    let exit_discipline = match response.answers.get(EXIT_DISCIPLINE_QUESTION_ID) {
        Some(Answer::Score { score, .. }) => *score,
        Some(other) => {
            return Err(TypeSafeError::Parse(format!(
                "{EXIT_DISCIPLINE_QUESTION_ID} answered with {} instead of a score",
                other.kind()
            )))
        }
        None => {
            return Err(TypeSafeError::Parse(format!(
                "no answer for {EXIT_DISCIPLINE_QUESTION_ID}"
            )))
        }
    };

    let stored = NoteAttribution {
        model: response.model.clone(),
        setup_tag,
        setup_confidence: *confidence,
        rule_break_noul,
        exit_discipline,
    };
    let audit = AttributionAudit {
        model: response.model.clone(),
        usage: response.usage,
        answers: response.answers,
    };

    Ok(Attribution::Attributed { stored, audit })
}

// ─────────────────────────────────────────────────────────────────────────
// Orchestration
// ─────────────────────────────────────────────────────────────────────────

/// Decides the attribution without touching the database.
///
/// Separated from [`attribute_note`] so every branch is reachable from a unit
/// test with a scripted client and no pool.
pub async fn decide_attribution(
    client: &dyn SystemOneClient,
    context: &NoteContext,
    candidates: &[TagCandidate],
) -> Attribution {
    if context.notes.trim().is_empty() {
        // No model call: there is nothing to read, and the status records why
        // the columns are empty rather than spending a request to learn it.
        return Attribution::NoNote;
    }

    let request = build_attribution_request(context, candidates);
    match client.judge(&request, CallPolicy::post_trade()).await {
        Ok(response) => interpret_attribution(response, candidates).unwrap_or_else(|err| {
            tracing::warn!(
                trade_id = %context.id,
                error = %err,
                "typesafe: attribution answer unusable"
            );
            Attribution::Failed(err)
        }),
        Err(err) => {
            tracing::warn!(
                trade_id = %context.id,
                error = %err,
                "typesafe: attribution call failed"
            );
            Attribution::Failed(err)
        }
    }
}

/// Attributes one note, then persists the outcome.
///
/// Idempotent: re-running overwrites the columns and appends a second audit
/// row. Re-running is the expected case, not an error, because the trader may
/// edit the note and because a lost spawned task leaves a retryable state.
pub async fn attribute_note(
    pool: &PgPool,
    client: &dyn SystemOneClient,
    context: &NoteContext,
    candidates: &[TagCandidate],
) -> Result<Attribution, sqlx::Error> {
    let attribution = decide_attribution(client, context, candidates).await;

    match &attribution {
        Attribution::Attributed { stored, audit } => {
            persist_attributed(pool, context, stored, audit).await?;
        }
        Attribution::NoNote => persist_status(pool, context, AttributionStatus::NoNote).await?,
        Attribution::Failed(err) => persist_failure(pool, context, err).await?,
    }

    Ok(attribution)
}

// ─────────────────────────────────────────────────────────────────────────
// Persistence
// ─────────────────────────────────────────────────────────────────────────

/// Records a terminal state that carries no attribution.
async fn persist_status(
    pool: &PgPool,
    context: &NoteContext,
    status: AttributionStatus,
) -> Result<(), sqlx::Error> {
    debug_assert_ne!(status, AttributionStatus::Attributed);
    sqlx::query(
        "UPDATE journal_trades SET judgment_status = $1, updated_at = NOW() \
         WHERE id = $2 AND user_id = $3",
    )
    .bind(status.as_db_str())
    .bind(context.id)
    .bind(context.user_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Writes the projection and the audit row in one transaction.
///
/// Atomic on purpose: a stored attribution with no audit row would be
/// unfalsifiable, and an audit row with no stored attribution would make the
/// columns look unread while the log claims otherwise.
async fn persist_attributed(
    pool: &PgPool,
    context: &NoteContext,
    stored: &NoteAttribution,
    audit: &AttributionAudit,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE journal_trades SET \
             judgment_status = 'attributed', judgment_model = $1, judgment_at = NOW(), \
             judgment_setup_tag = $2, judgment_setup_confidence = $3, \
             judgment_rule_break_noul = $4, judgment_exit_discipline = $5, \
             updated_at = NOW() \
         WHERE id = $6 AND user_id = $7",
    )
    .bind(&stored.model)
    .bind(stored.setup_tag.as_deref())
    .bind(stored.setup_confidence)
    .bind(stored.rule_break_noul)
    .bind(stored.exit_discipline)
    .bind(context.id)
    .bind(context.user_id)
    .execute(&mut *tx)
    .await?;

    // The raw reply, verbatim. `answers` keys are the question ids, so the row
    // is self-describing without duplicating the state we sent.
    let payload = json!({
        "kind": "note_attribution",
        "trade_id": context.id,
        "status": AttributionStatus::Attributed.as_db_str(),
        "model": audit.model,
        "usage": audit.usage,
        "answers": audit.answers,
    });

    insert_audit_row(&mut tx, context, payload).await?;

    tx.commit().await?;
    Ok(())
}

/// Records a failed attempt. The error string names the failure class so
/// service failures stay separable from model errors.
async fn persist_failure(
    pool: &PgPool,
    context: &NoteContext,
    err: &TypeSafeError,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "UPDATE journal_trades SET judgment_status = 'failed', updated_at = NOW() \
         WHERE id = $1 AND user_id = $2",
    )
    .bind(context.id)
    .bind(context.user_id)
    .execute(&mut *tx)
    .await?;

    let payload = json!({
        "kind": "note_attribution",
        "trade_id": context.id,
        "status": AttributionStatus::Failed.as_db_str(),
        "error": err.to_string(),
        "retryable": err.is_retryable(),
    });

    insert_audit_row(&mut tx, context, payload).await?;

    tx.commit().await?;
    Ok(())
}

/// Appends one row to the append-only event log.
///
/// Written directly rather than through `TradeEventWriter`: that path is driven
/// by the engine's closed `TradeEventType` enum, and routing a judgement
/// through it would put a model call on the engine's channel. Writing inside
/// the caller's transaction also makes the state and its audit land together.
async fn insert_audit_row(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    context: &NoteContext,
    payload: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO trade_events (event_type, group_id, user_id, symbol, payload) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(EVENT_TYPE)
    .bind(context.trade_group_id)
    .bind(context.user_id)
    .bind(&context.symbol)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// `trade_events.event_type` value. Lowercase snake case, matching the engine's
/// convention for the same column.
pub const EVENT_TYPE: &str = "judgment_attribution";

// ─────────────────────────────────────────────────────────────────────────
// Trigger
// ─────────────────────────────────────────────────────────────────────────

/// Fires UC-3 after a note is saved.
///
/// Spawned rather than awaited: the post-trade policy allows a 30s timeout with
/// retries, and a trader should not wait on a judgement to get their note
/// saved. Nothing downstream blocks on the result.
///
/// Returns without spawning when no client is configured, so a disabled
/// integration costs one `Option` read and leaves the row at `not_attempted`,
/// which is the retryable state.
// ponytail: two hooks, not a sweep. Both note arrival paths are covered
// (the notes route and the JNL-20 draft merge), but a spawn lost to a restart
// leaves the row at `not_attempted` with nothing to retry it. A sweeper over
// `judgment_status = 'not_attempted'` is the fix when a backfill of existing
// rows or recovery from dropped spawns actually matters.
pub fn spawn_note_attribution(
    client: Option<std::sync::Arc<dyn SystemOneClient>>,
    pool: PgPool,
    trade_id: Uuid,
    user_id: Uuid,
) {
    let Some(client) = client else {
        tracing::debug!(%trade_id, "typesafe: attribution skipped, no client configured");
        return;
    };

    tokio::spawn(async move {
        let context = match load_note_context(&pool, trade_id, user_id).await {
            Ok(Some(context)) => context,
            Ok(None) => {
                tracing::warn!(%trade_id, "typesafe: attribution found no trade");
                return;
            }
            Err(e) => {
                tracing::error!(error = %e, %trade_id, "typesafe: attribution load failed");
                return;
            }
        };

        // Loaded after the trade so a missing vocabulary cannot waste a load on
        // a trade that does not exist.
        let candidates = match load_tag_candidates(&pool, user_id).await {
            Ok(candidates) => candidates,
            Err(e) => {
                tracing::error!(error = %e, "typesafe: setup tag load failed");
                return;
            }
        };

        match attribute_note(&pool, client.as_ref(), &context, &candidates).await {
            Ok(outcome) => tracing::debug!(
                %trade_id,
                outcome = ?outcome,
                "typesafe: attribution finished"
            ),
            Err(e) => {
                tracing::error!(error = %e, %trade_id, "typesafe: attribution write failed")
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;
    use crate::services::typesafe::client::MockSystemOneClient;
    use crate::services::typesafe::types::MODEL_PINNED;

    fn context(notes: &str) -> NoteContext {
        NoteContext {
            id: Uuid::nil(),
            user_id: Uuid::nil(),
            trade_group_id: None,
            symbol: "BTCUSDT".to_string(),
            side: "LONG".to_string(),
            notes: notes.to_string(),
            setup_tag: Some("breakout".to_string()),
            close_reason: Some("tp".to_string()),
            duration_secs: 5_400,
            r_multiple: Some(dec!(2.4)),
        }
    }

    fn candidates() -> Vec<TagCandidate> {
        vec![
            TagCandidate {
                tag: "breakout".to_string(),
                uses: 14,
            },
            TagCandidate {
                tag: "mean_reversion".to_string(),
                uses: 9,
            },
        ]
    }

    /// A client that answers all three questions.
    fn answering(tag: &str, rule_break: Decimal, exit: Decimal) -> MockSystemOneClient {
        MockSystemOneClient::new(Ok(SystemOneResponse {
            model: MODEL_PINNED.to_string(),
            answers: [
                (
                    TAG_QUESTION_ID.to_string(),
                    Answer::Choice {
                        choice: tag.to_string(),
                        probabilities: [("breakout".to_string(), dec!(0.9))]
                            .into_iter()
                            .collect(),
                        confidence: dec!(0.88),
                    },
                ),
                (
                    RULE_BREAK_QUESTION_ID.to_string(),
                    Answer::Noul { noul: rule_break },
                ),
                (
                    EXIT_DISCIPLINE_QUESTION_ID.to_string(),
                    Answer::Score {
                        score: exit,
                        legend: BTreeMap::new(),
                        probabilities: BTreeMap::new(),
                        confidence: dec!(0.7),
                    },
                ),
            ]
            .into_iter()
            .collect(),
            usage: Usage {
                input_tokens: 300,
                output_tokens: 12,
            },
        }))
    }

    // ── Request shape ────────────────────────────────────────────────────

    #[test]
    fn request_asks_the_three_primitives_once_each() {
        let request = build_attribution_request(&context("held through the retest"), &candidates());
        assert!(request.validate().is_ok(), "request must be wire-valid");
        assert_eq!(request.questions.len(), 3);
        assert_eq!(request.questions[TAG_QUESTION_ID].kind(), "choice");
        assert_eq!(request.questions[RULE_BREAK_QUESTION_ID].kind(), "noul");
        assert_eq!(request.questions[EXIT_DISCIPLINE_QUESTION_ID].kind(), "score");
    }

    #[test]
    fn every_question_states_its_judgement_without_relying_on_its_id() {
        // The vendor does not send question ids to the model, so a bare label
        // would be judged as nothing. Substantive instructions and an explicit
        // state path are the only thing carrying meaning.
        let request = build_attribution_request(&context("note text"), &candidates());
        for (id, question) in &request.questions {
            let instructions = match question {
                Question::Noul { instructions, .. }
                | Question::Choice { instructions, .. }
                | Question::Score { instructions, .. } => instructions,
            };
            assert!(
                instructions.len() > 150,
                "{id} instructions are too thin to judge from"
            );
            assert!(instructions.contains('`'), "{id} must name a state path");
        }
    }

    #[test]
    fn request_offers_every_candidate_plus_the_no_match_option() {
        let offered = candidates();
        let request = build_attribution_request(&context("note text"), &offered);
        let Question::Choice { criteria, .. } = &request.questions[TAG_QUESTION_ID] else {
            panic!("the tag question must be a choice");
        };
        for candidate in &offered {
            assert!(
                criteria.contains_key(&candidate.tag),
                "the model cannot pick a tag that was not offered"
            );
        }
        assert!(criteria.contains_key(crate::services::typesafe::service::NOVEL_TAG_OPTION));
    }

    #[test]
    fn a_tag_equal_to_the_sentinel_is_never_offered() {
        let offered = vec![
            TagCandidate {
                tag: crate::services::typesafe::service::NOVEL_TAG_OPTION.to_string(),
                uses: 3,
            },
            TagCandidate {
                tag: "breakout".to_string(),
                uses: 14,
            },
        ];
        let request = build_attribution_request(&context("note text"), &offered);
        let Question::Choice { criteria, .. } = &request.questions[TAG_QUESTION_ID] else {
            panic!("the tag question must be a choice");
        };
        assert_eq!(criteria.len(), 2, "one real tag plus the sentinel");
    }

    #[test]
    fn request_state_names_every_field_the_instructions_reference() {
        let request = build_attribution_request(&context("held through the retest"), &candidates());
        assert_eq!(request.state["note"], "held through the retest");
        assert_eq!(request.state["trade"]["symbol"], "BTCUSDT");
        assert_eq!(request.state["trade"]["side"], "LONG");
        assert_eq!(request.state["trade"]["setup_tag"], "breakout");
        assert_eq!(request.state["trade"]["close_reason"], "tp");
        // Seconds are converted so the model reads a duration, not a counter.
        assert_eq!(request.state["trade"]["duration_minutes"], 90);
        assert_eq!(request.state["existing_tags"][0]["tag"], "breakout");
        assert_eq!(request.state["existing_tags"][0]["closed_trades"], 14);
    }

    #[test]
    fn score_levels_match_the_code_owned_constant() {
        let request = build_attribution_request(&context("note text"), &candidates());
        let Question::Score { criteria, .. } = &request.questions[EXIT_DISCIPLINE_QUESTION_ID]
        else {
            panic!("exit discipline must be a score");
        };
        assert_eq!(criteria.len(), EXIT_DISCIPLINE_LEVELS.len());
        assert_eq!(criteria[0], EXIT_DISCIPLINE_LEVELS[0]);
        // The vendor requires at least two levels; more than one is what lets
        // the answer land between them.
        assert!(criteria.len() >= 2);
    }

    // ── Interpretation ───────────────────────────────────────────────────

    #[tokio::test]
    async fn attribution_keeps_the_projection_and_the_raw_reply_together() {
        let client = answering("breakout", dec!(0.18), dec!(1.4));
        let got = decide_attribution(&client, &context("held through the retest"), &candidates())
            .await;

        let Attribution::Attributed { stored, audit } = got else {
            panic!("expected an attribution, got {got:?}");
        };
        assert_eq!(stored.model, MODEL_PINNED);
        assert_eq!(stored.setup_tag.as_deref(), Some("breakout"));
        assert_eq!(stored.setup_confidence, dec!(0.88));
        assert_eq!(stored.rule_break_noul, dec!(0.18));
        assert_eq!(stored.exit_discipline, dec!(1.4));

        // The audit keeps the vendor's own answer, not our projection of it.
        assert_eq!(audit.answers.len(), 3);
        assert!(matches!(
            audit.answers.get(RULE_BREAK_QUESTION_ID),
            Some(Answer::Noul { noul }) if *noul == dec!(0.18)
        ));
        assert_eq!(audit.usage.input_tokens, 300);
    }

    #[tokio::test]
    async fn a_raw_probability_survives_unthresholded() {
        // 0.5 is an even split, not medium intensity. Nothing in this path may
        // collapse it to a verdict, because each consumer owns its cutoff.
        let client = answering("breakout", dec!(0.5), dec!(1.0));
        let got = decide_attribution(&client, &context("note text"), &candidates()).await;
        let Attribution::Attributed { stored, .. } = got else {
            panic!("an even split is still an answer");
        };
        assert_eq!(stored.rule_break_noul, dec!(0.5));
    }

    #[tokio::test]
    async fn the_no_match_option_is_an_answer_not_a_gap() {
        let no_match = crate::services::typesafe::service::NOVEL_TAG_OPTION;
        let client = answering(no_match, dec!(0.1), dec!(0.2));
        let got = decide_attribution(&client, &context("note text"), &candidates()).await;
        let Attribution::Attributed { stored, .. } = got else {
            panic!("a novel setup is still an attribution");
        };
        assert_eq!(stored.setup_tag, None);
        // The other two axes still land: one unanswered question does not
        // invalidate the two that were answered.
        assert_eq!(stored.exit_discipline, dec!(0.2));
    }

    #[tokio::test]
    async fn an_unoffered_tag_is_discarded_rather_than_stored() {
        let client = answering("invented_tag", dec!(0.1), dec!(0.2));
        let got = decide_attribution(&client, &context("note text"), &candidates()).await;
        match got {
            Attribution::Failed(TypeSafeError::Parse(msg)) => {
                assert!(msg.contains("invented_tag"));
            }
            other => panic!("expected a discarded answer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_partial_answer_is_rejected_rather_than_stored() {
        // Two of three axes is a different record from three of three, so it is
        // not merged into the same columns.
        let client = MockSystemOneClient::new(Ok(SystemOneResponse {
            model: MODEL_PINNED.to_string(),
            answers: [(
                TAG_QUESTION_ID.to_string(),
                Answer::Choice {
                    choice: "breakout".to_string(),
                    probabilities: BTreeMap::new(),
                    confidence: dec!(0.9),
                },
            )]
            .into_iter()
            .collect(),
            usage: Usage::default(),
        }));
        let got = decide_attribution(&client, &context("note text"), &candidates()).await;
        match got {
            Attribution::Failed(TypeSafeError::Parse(msg)) => {
                assert!(msg.contains(RULE_BREAK_QUESTION_ID));
            }
            other => panic!("expected a rejected partial answer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_wrong_primitive_is_rejected() {
        // Asking for a score and receiving a noul means our question and the
        // vendor's answer disagree about the shape; storing it would be a lie.
        let client = MockSystemOneClient::new(Ok(SystemOneResponse {
            model: MODEL_PINNED.to_string(),
            answers: [
                (
                    TAG_QUESTION_ID.to_string(),
                    Answer::Choice {
                        choice: "breakout".to_string(),
                        probabilities: BTreeMap::new(),
                        confidence: dec!(0.9),
                    },
                ),
                (
                    RULE_BREAK_QUESTION_ID.to_string(),
                    Answer::Noul { noul: dec!(0.3) },
                ),
                (
                    EXIT_DISCIPLINE_QUESTION_ID.to_string(),
                    Answer::Noul { noul: dec!(0.3) },
                ),
            ]
            .into_iter()
            .collect(),
            usage: Usage::default(),
        }));
        let got = decide_attribution(&client, &context("note text"), &candidates()).await;
        match got {
            Attribution::Failed(TypeSafeError::Parse(msg)) => {
                assert!(msg.contains(EXIT_DISCIPLINE_QUESTION_ID));
                assert!(msg.contains("noul"));
            }
            other => panic!("expected a rejected primitive, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_blank_note_never_reaches_the_model() {
        // Scripted as a failure: if the client were consulted at all, the
        // outcome could not be NoNote.
        let client = MockSystemOneClient::new(Err(TypeSafeError::Timeout));
        let got = decide_attribution(&client, &context("   \n\t "), &candidates()).await;
        assert_eq!(got, Attribution::NoNote);
    }

    #[tokio::test]
    async fn a_transport_failure_is_failed_and_keeps_its_reason() {
        let client = MockSystemOneClient::new(Err(TypeSafeError::RateLimit {
            retry_after: Some(std::time::Duration::from_secs(30)),
        }));
        let got = decide_attribution(&client, &context("note text"), &candidates()).await;
        match got {
            Attribution::Failed(TypeSafeError::RateLimit { retry_after: Some(_) }) => {}
            other => panic!("expected Failed with the transport error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_trader_with_no_vocabulary_still_gets_the_other_two_axes() {
        // An empty option set is not sendable as a Choice, so the tag question
        // must degrade without taking the rule-break and exit answers with it.
        let client = answering(
            crate::services::typesafe::service::NOVEL_TAG_OPTION,
            dec!(0.4),
            dec!(0.9),
        );
        let got = decide_attribution(&client, &context("note text"), &[]).await;
        let Attribution::Attributed { stored, .. } = got else {
            panic!("expected an attribution, got {got:?}");
        };
        assert_eq!(stored.setup_tag, None);
        assert_eq!(stored.rule_break_noul, dec!(0.4));
    }

    // ── Stored contract ──────────────────────────────────────────────────

    #[test]
    fn status_strings_match_the_migration_check_constraint() {
        // The CHECK constraint in
        // `20260605000000_judgment_attribution.up.sql` lists these four values.
        // Drift here is a runtime constraint violation, so it is pinned.
        assert_eq!(AttributionStatus::NotAttempted.as_db_str(), "not_attempted");
        assert_eq!(AttributionStatus::NoNote.as_db_str(), "no_note");
        assert_eq!(AttributionStatus::Attributed.as_db_str(), "attributed");
        assert_eq!(AttributionStatus::Failed.as_db_str(), "failed");
    }

    #[test]
    fn event_type_follows_the_existing_column_convention() {
        // `trade_events.event_type` is lowercase snake case for every other
        // writer, so an analytics query can group across all of them.
        assert!(!EVENT_TYPE.is_empty());
        assert!(EVENT_TYPE
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_'));
    }

    #[test]
    fn audit_payload_round_trips_the_raw_answers() {
        // The audit is only useful if a later threshold change can be evaluated
        // against what the model actually said, so the payload must carry the
        // answers verbatim rather than the projection.
        let answers: BTreeMap<String, Answer> = [(
            RULE_BREAK_QUESTION_ID.to_string(),
            Answer::Noul { noul: dec!(0.42) },
        )]
        .into_iter()
        .collect();
        let audit = AttributionAudit {
            model: MODEL_PINNED.to_string(),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 2,
            },
            answers,
        };

        let value = serde_json::to_value(&audit).expect("audit serialises");
        assert_eq!(value["model"], MODEL_PINNED);
        assert_eq!(value["usage"]["input_tokens"], 10);
        assert_eq!(value["answers"][RULE_BREAK_QUESTION_ID]["type"], "noul");

        // And it deserialises back to the same answer, which is what makes the
        // stored row re-readable.
        let back: BTreeMap<String, Answer> =
            serde_json::from_value(value["answers"].clone()).expect("answers round-trip");
        assert!(matches!(
            back.get(RULE_BREAK_QUESTION_ID),
            Some(Answer::Noul { noul }) if *noul == dec!(0.42)
        ));
    }

    #[test]
    fn failure_payload_classifies_the_error() {
        // The four failure classes have to stay separable in the log, so the
        // payload carries both the reason and whether a retry could help.
        let retryable = json!({
            "status": AttributionStatus::Failed.as_db_str(),
            "error": TypeSafeError::RateLimit { retry_after: None }.to_string(),
            "retryable": TypeSafeError::RateLimit { retry_after: None }.is_retryable(),
        });
        assert_eq!(retryable["status"], "failed");
        assert_eq!(retryable["retryable"], true);

        let terminal = TypeSafeError::Unauthorized;
        assert!(!terminal.is_retryable());
    }
}
