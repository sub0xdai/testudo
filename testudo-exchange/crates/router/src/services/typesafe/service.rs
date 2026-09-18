//! TS-01 — Judgement services.
//!
//! Each service turns a domain question into one TypeSafe request, then
//! interprets the answer in code. The model supplies the judgement; code
//! supplies the candidates, the thresholds, and every action taken on the
//! result.
//!
//! UC-1 lives here: setup tag resolution. Design and scope in
//! `docs/plans/typesafe-jev-sniper-integration.md`.

// @anchor exchange:router:typesafe-service
// @tags api

use std::collections::BTreeMap;

use common_utils::journal::TradeSide;
use rust_decimal::Decimal;
use serde_json::json;

use super::client::SystemOneClient;
use super::types::{Answer, CallPolicy, Question, SystemOneRequest, TypeSafeError};

/// Question id for setup tag resolution.
///
/// Ids are for our code only and are never sent to the model, which is why
/// [`SETUP_TAG_INSTRUCTIONS`] states the judgement in full instead of relying
/// on the id.
pub const SETUP_TAG_QUESTION_ID: &str = "setup_tag";

/// Option offered when no existing tag fits.
///
/// A sentinel rather than a generated tag: the model may only pick from what
/// it is offered, and `keep what the trader typed` is not a tag.
pub const NOVEL_TAG_OPTION: &str = "__new_setup__";

/// Upper bound on options sent. Each option is a tag plus a rubric, so this
/// is a direct token cost on a path the user is waiting on.
pub const MAX_CANDIDATES: usize = 24;

/// Minimum distribution concentration to accept a resolution.
///
/// Below this the model saw more than one plausible fit, and a wrong label
/// would pull a different setup's history into this trade's calibration.
/// Keeping the trader's own text costs one calibration lookup; a wrong label
/// corrupts one. Take the cheap loss.
// ponytail: 0.60 is a placeholder. Set it from the UC-1 replay the design
// review calls for, before TYPESAFE_ENABLED is turned on.
pub const MIN_RESOLUTION_CONFIDENCE: Decimal = Decimal::from_parts(60, 0, 0, false, 2);

/// The judgement, stated in full.
///
/// Explicit state paths, an explicit tie-break rule, and an explicit
/// no-match instruction: the model cannot see our intent, only this text.
const SETUP_TAG_INSTRUCTIONS: &str = "\
The trader typed the setup tag in `typed_tag` while preparing the trade in \
`trade`. `existing_tags` lists setup tags already used on their closed trades, \
each with the number of closed trades carrying it. `existing_tags` is the only \
source of tags: do not invent one. Which single option names the same setup idea \
as `typed_tag`? Judge the setup the trader means, not the spelling: a shortened, \
misspelled, or differently punctuated form of the same idea is the same setup. \
`trade.symbol`, `trade.side`, and `trade.timeframe` are context for that decision. \
Choose the option described as a new setup only when no listed tag describes the \
same setup idea.";

/// One option offered to the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCandidate {
    pub tag: String,
    /// Closed trades carrying this tag. `0` when unknown.
    pub uses: u32,
}

/// What the tag judgement needs to know about the trade in flight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagResolutionInput {
    /// What the trader typed into the setup tag field.
    pub typed_tag: String,
    /// The trader's own vocabulary, most recently used first.
    pub known_tags: Vec<TagCandidate>,
    pub symbol: String,
    pub side: TradeSide,
    pub timeframe: String,
}

/// Outcome of a setup tag resolution.
///
/// A sum type rather than `{ tag: Option<String>, resolved: bool }`, so a
/// caller cannot read a tag out of a resolution that never produced one.
#[derive(Debug, Clone, PartialEq)]
pub enum TagResolution {
    /// The model picked one of the trader's existing tags. Apply it verbatim:
    /// calibration matches history with `LOWER(setup_tag) = LOWER($2)`.
    Matched { tag: String, confidence: Decimal },
    /// Keep the trader's own text. Nothing fit, or the answer was too flat to
    /// trust.
    KeepAsTyped,
    /// Not produced. `reason` is `None` when the integration is not enabled,
    /// and `Some` when a call was attempted and failed. Both are "unavailable"
    /// to the caller, which shows nothing either way.
    Unavailable { reason: Option<TypeSafeError> },
}

impl TagResolution {
    /// The tag to write into `setup_tag`, when a resolution produced one.
    ///
    /// Kept private to this module: the route pattern-matches on the variant,
    /// which is what makes "keep what was typed" distinguishable from "no
    /// judgement". An accessor would flatten that distinction back into an
    /// `Option` and lose it.
    #[cfg(test)]
    fn resolved_tag(&self) -> Option<&str> {
        match self {
            Self::Matched { tag, .. } => Some(tag.as_str()),
            Self::KeepAsTyped | Self::Unavailable { .. } => None,
        }
    }
}

/// UC-1. Resolves a free-text setup tag onto one of the trader's own
/// historical tags, so the calibration lookup finds the history.
///
/// Returns [`TagResolution::Unavailable`] rather than an error: a judgement is
/// an enhancement, and no caller should have to handle its absence any
/// differently from its failure.
pub async fn resolve_setup_tag(
    client: Option<&dyn SystemOneClient>,
    input: &TagResolutionInput,
) -> TagResolution {
    let typed = input.typed_tag.trim();
    if typed.is_empty() {
        return TagResolution::KeepAsTyped;
    }

    // Code owns the exact case. Only the fuzzy case needs a model, which
    // keeps the common path (the trader accepted a suggestion) free.
    if let Some(exact) = input
        .known_tags
        .iter()
        .find(|c| c.tag.eq_ignore_ascii_case(typed))
    {
        return TagResolution::Matched {
            tag: exact.tag.clone(),
            confidence: Decimal::ONE,
        };
    }

    let candidates = generate_candidates(typed, &input.known_tags);
    if candidates.is_empty() {
        // Nothing to choose from, so no model call can help and the vendor
        // rejects an empty option set. Checked before the client so that
        // "nothing to resolve" never reads as "integration unavailable".
        return TagResolution::KeepAsTyped;
    }

    let Some(client) = client else {
        return TagResolution::Unavailable { reason: None };
    };

    let request = build_request(input, &candidates);
    match client.judge(&request, CallPolicy::pre_trade()).await {
        Ok(response) => interpret(&response, &candidates),
        Err(err) => {
            tracing::warn!(error = %err, "typesafe: setup tag resolution failed");
            TagResolution::Unavailable { reason: Some(err) }
        }
    }
}

/// Builds a judgement from a domain input. Separated from the call so the
/// request can be asserted without a client.
fn build_request(input: &TagResolutionInput, candidates: &[TagCandidate]) -> SystemOneRequest {
    let options = tag_options(candidates.iter(), "No listed tag describes the same setup idea.");

    let state = json!({
        "typed_tag": input.typed_tag.trim(),
        "trade": {
            "symbol": input.symbol,
            "side": side_label(&input.side),
            "timeframe": input.timeframe,
        },
        "existing_tags": candidates
            .iter()
            .map(|c| json!({ "tag": c.tag, "closed_trades": c.uses }))
            .collect::<Vec<_>>(),
    });

    SystemOneRequest {
        state,
        questions: [(
            SETUP_TAG_QUESTION_ID.to_string(),
            Question::choice(SETUP_TAG_INSTRUCTIONS, options),
        )]
        .into_iter()
        .collect(),
    }
}

/// Candidate generation, in code.
///
/// The model may only choose from what it is offered, so an omitted candidate
/// can never be selected. Tiering is deliberate and stable: a normalised exact
/// match first, then prefix, then substring, then a loose last tier.
fn generate_candidates(typed: &str, known: &[TagCandidate]) -> Vec<TagCandidate> {
    let needle = normalize(typed);
    if needle.is_empty() {
        return Vec::new();
    }

    let mut exact: Vec<TagCandidate> = Vec::new();
    let mut prefix: Vec<TagCandidate> = Vec::new();
    let mut contains: Vec<TagCandidate> = Vec::new();
    let mut loose: Vec<TagCandidate> = Vec::new();

    for candidate in without_sentinel_collisions(known) {
        let tag = normalize(&candidate.tag);
        if tag.is_empty() {
            continue;
        }
        if tag == needle {
            exact.push(candidate.clone());
        } else if tag.starts_with(&needle) || needle.starts_with(&tag) {
            prefix.push(candidate.clone());
        } else if tag.contains(&needle) || needle.contains(&tag) {
            contains.push(candidate.clone());
        } else if loosely_matches(&needle, &tag) {
            loose.push(candidate.clone());
        }
    }

    exact.extend(prefix);
    exact.extend(contains);
    exact.extend(loose);
    exact.truncate(MAX_CANDIDATES);
    exact
}

/// Last-tier recall for dropped or added characters.
///
/// Prefix and substring matching both miss `brkout` for `breakout`. The model
/// cannot select an option that was never offered, so recall matters more here
/// than precision: a spare candidate costs a few tokens, a missing one loses
/// the resolution entirely. The length guard stops a long, largely unrelated
/// tag from riding along on character overlap alone.
fn loosely_matches(needle: &str, tag: &str) -> bool {
    let (short_len, long_len) = {
        let (n, t) = (needle.chars().count(), tag.chars().count());
        (n.min(t), n.max(t))
    };
    if short_len < 4 || long_len > short_len * 2 {
        return false;
    }
    is_subsequence(needle, tag) || is_subsequence(tag, needle)
}

/// True when every character of `short` appears in `long` in order.
fn is_subsequence(short: &str, long: &str) -> bool {
    let mut long_chars = long.chars();
    short.chars().all(|c| long_chars.any(|l| l == c))
}

/// Casefolded with separators removed: `Mean-Reversion` and `mean_reversion`
/// are the same tag as far as a trader is concerned.
fn normalize(tag: &str) -> String {
    tag.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Drops any tag that collides with the no-match sentinel.
///
/// Shared by every Choice that offers tags as options, in both UC-1 and UC-3.
/// The sentinel is compared literally against option keys, so a real tag equal
/// to it would make the answer ambiguous: the model could mean either.
pub(super) fn without_sentinel_collisions(known: &[TagCandidate]) -> Vec<&TagCandidate> {
    known
        .iter()
        .filter(|candidate| {
            let collides = candidate.tag == NOVEL_TAG_OPTION;
            if collides {
                tracing::warn!(
                    "typesafe: skipping a tag that collides with the no-match sentinel"
                );
            }
            !collides
        })
        .collect()
}

/// Builds the option list for a tag Choice: every candidate, then the
/// no-match sentinel.
///
/// The model may only pick from what it is offered, so this is the only place
/// a tag becomes selectable. Takes borrowed candidates so callers that already
/// hold a filtered view do not have to clone it back to an owned slice.
pub(super) fn tag_options<'a>(
    candidates: impl IntoIterator<Item = &'a TagCandidate>,
    novel_rubric: &str,
) -> Vec<(String, Option<String>)> {
    candidates
        .into_iter()
        .map(|c| (c.tag.clone(), Some(rubric(c))))
        .chain(std::iter::once((
            NOVEL_TAG_OPTION.to_string(),
            Some(novel_rubric.to_string()),
        )))
        .collect()
}

/// Renders a candidate's sample size into its option rubric.
fn rubric(candidate: &TagCandidate) -> String {
    match candidate.uses {
        0 => format!("{} (no closed trades yet)", candidate.tag),
        1 => format!("{} (1 closed trade)", candidate.tag),
        n => format!("{} ({n} closed trades)", candidate.tag),
    }
}

fn side_label(side: &TradeSide) -> &'static str {
    match side {
        TradeSide::Long => "long",
        TradeSide::Short => "short",
    }
}

/// Turns a response into a resolution, applying the code-owned threshold.
fn interpret(
    response: &super::types::SystemOneResponse,
    candidates: &[TagCandidate],
) -> TagResolution {
    let Some(answer) = response.answers.get(SETUP_TAG_QUESTION_ID) else {
        return TagResolution::Unavailable {
            reason: Some(TypeSafeError::Parse("no answer for setup_tag".into())),
        };
    };

    let Answer::Choice {
        choice,
        probabilities,
        confidence,
    } = answer
    else {
        return TagResolution::Unavailable {
            reason: Some(TypeSafeError::Parse(format!(
                "expected a choice answer, got {}",
                answer.kind()
            ))),
        };
    };

    if choice == NOVEL_TAG_OPTION {
        return TagResolution::KeepAsTyped;
    }

    // The model is only supposed to pick an offered option. A name we did not
    // offer is discarded rather than written into the journal as a tag.
    let Some(candidate) = candidates.iter().find(|c| &c.tag == choice) else {
        tracing::warn!(choice = %choice, "typesafe: answer named an option we did not offer");
        return TagResolution::Unavailable {
            reason: Some(TypeSafeError::Parse(format!("unoffered option {choice}"))),
        };
    };

    if *confidence < MIN_RESOLUTION_CONFIDENCE {
        tracing::debug!(
            confidence = %confidence,
            threshold = %MIN_RESOLUTION_CONFIDENCE,
            "typesafe: resolution below threshold, keeping the typed tag"
        );
        return TagResolution::KeepAsTyped;
    }

    tracing::debug!(
        tag = %candidate.tag,
        confidence = %confidence,
        model = %response.model,
        probabilities = ?probabilities,
        "typesafe: setup tag resolved"
    );

    TagResolution::Matched {
        tag: candidate.tag.clone(),
        confidence: *confidence,
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;
    use crate::services::typesafe::client::MockSystemOneClient;
    use crate::services::typesafe::types::{Answer, SystemOneResponse, Usage, MODEL_PINNED};

    fn candidate(tag: &str, uses: u32) -> TagCandidate {
        TagCandidate {
            tag: tag.to_string(),
            uses,
        }
    }

    fn input(typed: &str, known: Vec<TagCandidate>) -> TagResolutionInput {
        TagResolutionInput {
            typed_tag: typed.to_string(),
            known_tags: known,
            symbol: "BTCUSDT".to_string(),
            side: TradeSide::Long,
            timeframe: "15m".to_string(),
        }
    }

    fn vocab() -> Vec<TagCandidate> {
        vec![
            candidate("breakout", 14),
            candidate("mean_reversion", 9),
            candidate("funding_skew", 3),
        ]
    }

    fn answering_choice(choice: &str, confidence: Decimal) -> MockSystemOneClient {
        MockSystemOneClient::new(Ok(SystemOneResponse {
            model: MODEL_PINNED.to_string(),
            answers: [(
                SETUP_TAG_QUESTION_ID.to_string(),
                Answer::Choice {
                    choice: choice.to_string(),
                    probabilities: [("breakout".to_string(), confidence)]
                        .into_iter()
                        .collect(),
                    confidence,
                },
            )]
            .into_iter()
            .collect(),
            usage: Usage::default(),
        }))
    }

    #[tokio::test]
    async fn exact_match_resolves_without_calling_the_model() {
        // No client at all: an exact match must never need one.
        let got = resolve_setup_tag(None, &input("breakout", vocab())).await;
        assert_eq!(
            got,
            TagResolution::Matched {
                tag: "breakout".to_string(),
                confidence: Decimal::ONE
            }
        );
    }

    #[tokio::test]
    async fn the_code_owned_exact_path_mirrors_the_calibration_match_rule() {
        // Calibration finds history with LOWER(setup_tag) = LOWER($2), so only
        // a pure case difference is safe to accept without the model. A
        // separator difference would resolve to a tag calibration cannot find,
        // so it must go to the model, which returns the stored spelling.
        for typed in ["Breakout", "BREAKOUT"] {
            let got = resolve_setup_tag(None, &input(typed, vocab())).await;
            assert!(
                got.resolved_tag().is_some(),
                "`{typed}` is a pure case difference and must resolve locally"
            );
        }
        for typed in ["MEAN-REVERSION", "mean reversion"] {
            let got = resolve_setup_tag(None, &input(typed, vocab())).await;
            assert_eq!(
                got,
                TagResolution::Unavailable { reason: None },
                "`{typed}` must be sent to the model"
            );
        }
    }

    #[tokio::test]
    async fn a_fuzzy_tag_goes_to_the_model_and_comes_back_resolved() {
        let client = answering_choice("breakout", dec!(0.88));
        let got = resolve_setup_tag(Some(&client), &input("brkout", vocab())).await;
        assert_eq!(
            got,
            TagResolution::Matched {
                tag: "breakout".to_string(),
                confidence: dec!(0.88)
            }
        );
    }

    #[tokio::test]
    async fn a_flat_answer_keeps_the_typed_tag() {
        // Guard: if candidate generation regressed, this would pass vacuously
        // without the model ever being consulted.
        assert!(!generate_candidates("brkout", &vocab()).is_empty());
        // Below the threshold: a wrong label would corrupt calibration, so
        // the trader's own text stands.
        let client = answering_choice("breakout", dec!(0.41));
        let got = resolve_setup_tag(Some(&client), &input("brkout", vocab())).await;
        assert_eq!(got, TagResolution::KeepAsTyped);
    }

    #[tokio::test]
    async fn the_novel_option_keeps_the_typed_tag() {
        assert!(!generate_candidates("breakout_fade", &vocab()).is_empty());
        let client = answering_choice(NOVEL_TAG_OPTION, dec!(0.93));
        let got = resolve_setup_tag(Some(&client), &input("breakout_fade", vocab())).await;
        assert_eq!(got, TagResolution::KeepAsTyped);
    }

    #[tokio::test]
    async fn an_unoffered_option_is_discarded_not_written_to_the_journal() {
        let client = answering_choice("invented_tag", dec!(0.99));
        let got = resolve_setup_tag(Some(&client), &input("brkout", vocab())).await;
        match got {
            TagResolution::Unavailable { reason: Some(TypeSafeError::Parse(msg)) } => {
                assert!(msg.contains("invented_tag"));
            }
            other => panic!("expected a discarded answer, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_transport_failure_is_unavailable_and_carries_its_reason() {
        let client = MockSystemOneClient::new(Err(TypeSafeError::RateLimit {
            retry_after: Some(std::time::Duration::from_secs(30)),
        }));
        let got = resolve_setup_tag(Some(&client), &input("brkout", vocab())).await;
        match got {
            TagResolution::Unavailable { reason: Some(TypeSafeError::RateLimit { .. }) } => {}
            other => panic!("expected Unavailable with the transport error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_disabled_integration_is_unavailable_with_no_reason() {
        let got = resolve_setup_tag(None, &input("brkout", vocab())).await;
        assert_eq!(got, TagResolution::Unavailable { reason: None });
    }

    #[tokio::test]
    async fn an_empty_tag_is_never_sent_anywhere() {
        // No client, and the state would be meaningless: nothing to resolve.
        let got = resolve_setup_tag(None, &input("   ", vocab())).await;
        assert_eq!(got, TagResolution::KeepAsTyped);
    }

    #[tokio::test]
    async fn a_trader_with_no_history_keeps_their_own_text() {
        let got = resolve_setup_tag(None, &input("brkout", Vec::new())).await;
        assert_eq!(got, TagResolution::KeepAsTyped);
    }

    #[test]
    fn candidates_rank_exact_then_prefix_then_substring() {
        let known = vec![
            candidate("trend_continuation", 5),
            candidate("mean_reversion", 4),
            candidate("breakout_retest", 3),
            candidate("range_breakout", 2),
        ];
        let names: Vec<String> = generate_candidates("breakout", &known)
            .into_iter()
            .map(|c| c.tag)
            .collect();
        // `breakout_retest` and `range_breakout` contain the needle; the
        // others share no normalised overlap and are dropped.
        assert_eq!(names, vec!["breakout_retest", "range_breakout"]);
        assert!(
            !names.contains(&"mean_reversion".to_string()),
            "an unrelated tag must not be offered"
        );
    }

    #[test]
    fn candidates_are_capped_so_the_hot_path_stays_cheap() {
        let known: Vec<TagCandidate> = (0..80)
            .map(|i| candidate(&format!("breakout_{i}"), 1))
            .collect();
        assert_eq!(generate_candidates("breakout", &known).len(), MAX_CANDIDATES);
    }

    #[test]
    fn a_separator_only_difference_ranks_above_prefix_matches() {
        let known = vec![
            candidate("breakout_retest", 9),
            candidate("Mean-Reversion", 7),
        ];
        let names: Vec<String> = generate_candidates("mean reversion", &known)
            .into_iter()
            .map(|c| c.tag)
            .collect();
        assert_eq!(names, vec!["Mean-Reversion"]);
    }

    #[test]
    fn normalize_folds_case_and_drops_separators() {
        assert_eq!(normalize("Mean-Reversion"), "meanreversion");
        assert_eq!(normalize("  BREAKOUT  "), "breakout");
        assert_eq!(normalize("---"), "");
    }

    #[test]
    fn threshold_is_the_documented_placeholder() {
        // Pinned so a change to the constant is a deliberate edit.
        assert_eq!(MIN_RESOLUTION_CONFIDENCE, dec!(0.60));
    }

    #[test]
    fn request_offers_every_candidate_plus_a_new_setup_option() {
        let candidates = generate_candidates("brkout", &vocab());
        let request = build_request(&input("brkout", vocab()), &candidates);

        assert!(request.validate().is_ok());
        let question = request.questions.get(SETUP_TAG_QUESTION_ID).expect("asked");
        match question {
            Question::Choice { criteria, .. } => {
                for candidate in &candidates {
                    assert!(
                        criteria.contains_key(&candidate.tag),
                        "every candidate must be offered"
                    );
                }
                assert!(criteria.contains_key(NOVEL_TAG_OPTION));
                // The rubric carries the sample size the model needs to
                // prefer a well-established tag.
                assert!(criteria[&candidates[0].tag]
                    .as_deref()
                    .unwrap_or_default()
                    .contains("closed trades"));
            }
            other => panic!("UC-1 must ask a choice question, got {other:?}"),
        }
    }

    #[test]
    fn request_state_names_every_field_the_instructions_reference() {
        let candidates = generate_candidates("brkout", &vocab());
        let request = build_request(&input("brkout", vocab()), &candidates);

        assert_eq!(request.state["typed_tag"], "brkout");
        assert_eq!(request.state["trade"]["symbol"], "BTCUSDT");
        assert_eq!(request.state["trade"]["side"], "long");
        assert_eq!(request.state["trade"]["timeframe"], "15m");
        assert_eq!(request.state["existing_tags"][0]["tag"], "breakout");
        assert_eq!(request.state["existing_tags"][0]["closed_trades"], 14);
    }

    #[test]
    fn question_id_is_not_load_bearing_so_instructions_carry_the_meaning() {
        // The vendor does not send question ids to the model. If the
        // instructions ever shrink to a bare label, this catches it.
        assert!(SETUP_TAG_INSTRUCTIONS.contains("typed_tag"));
        assert!(SETUP_TAG_INSTRUCTIONS.contains("existing_tags"));
        assert!(SETUP_TAG_INSTRUCTIONS.len() > 200);
    }

    #[test]
    fn candidates_include_a_tag_typoed_by_dropping_characters() {
        // `brkout` for `breakout`: prefix and substring matching both miss
        // it, and the model cannot pick a tag that was never offered.
        let names: Vec<String> = generate_candidates("brkout", &vocab())
            .into_iter()
            .map(|c| c.tag)
            .collect();
        assert_eq!(names, vec!["breakout"]);
    }

    #[test]
    fn loose_matching_is_bounded_so_unrelated_tags_do_not_flood_the_options() {
        let known = vec![
            candidate("trend_continuation", 5),
            candidate("mean_reversion", 4),
        ];
        assert!(
            generate_candidates("brkout", &known).is_empty(),
            "a long unrelated tag must not match on character overlap alone"
        );
        assert!(!loosely_matches("bt", "bitcoin_breakout"));
        assert!(loosely_matches("brkout", "breakout"));
        assert!(loosely_matches("breakouts", "breakout"));
    }

    #[test]
    fn a_tag_equal_to_the_no_match_sentinel_is_never_offered() {
        let known = vec![candidate(NOVEL_TAG_OPTION, 3), candidate("breakout", 14)];
        let offered: Vec<String> = generate_candidates("breakout", &known)
            .into_iter()
            .map(|c| c.tag)
            .collect();
        assert_eq!(offered, vec!["breakout"]);
    }

    #[test]
    fn interpretation_handles_a_wrong_answer_primitive() {
        let response = SystemOneResponse {
            model: MODEL_PINNED.to_string(),
            answers: [(
                SETUP_TAG_QUESTION_ID.to_string(),
                Answer::Noul { noul: dec!(0.9) },
            )]
            .into_iter()
            .collect(),
            usage: Usage::default(),
        };
        let candidates = generate_candidates("brkout", &vocab());
        match interpret(&response, &candidates) {
            TagResolution::Unavailable { reason: Some(TypeSafeError::Parse(msg)) } => {
                assert!(msg.contains("noul"));
            }
            other => panic!("expected a parse failure, got {other:?}"),
        }
    }

    #[test]
    fn interpretation_handles_a_missing_answer() {
        let response = SystemOneResponse {
            model: MODEL_PINNED.to_string(),
            answers: BTreeMap::new(),
            usage: Usage::default(),
        };
        assert!(matches!(
            interpret(&response, &[]),
            TagResolution::Unavailable { reason: Some(_) }
        ));
    }

    #[test]
    fn resolved_tag_only_exposes_a_tag_that_was_actually_produced() {
        assert_eq!(
            TagResolution::Matched {
                tag: "breakout".to_string(),
                confidence: Decimal::ONE
            }
            .resolved_tag(),
            Some("breakout")
        );
        assert_eq!(TagResolution::KeepAsTyped.resolved_tag(), None);
        assert_eq!(
            TagResolution::Unavailable { reason: None }.resolved_tag(),
            None
        );
    }

    #[test]
    fn keep_as_typed_and_unavailable_stay_distinguishable() {
        // The route depends on this: a resolved "no match" must not collapse
        // into "no judgement", or the client cannot tell a considered answer
        // from an unreachable model.
        assert_ne!(
            TagResolution::KeepAsTyped,
            TagResolution::Unavailable { reason: None }
        );
    }

    #[test]
    fn state_serialises_without_a_model_field() {
        // The client injects the model. Its absence here is the guarantee
        // that a call site cannot unpin it.
        let candidates = generate_candidates("brkout", &vocab());
        let request = build_request(&input("brkout", vocab()), &candidates);
        let value = serde_json::to_value(&request).expect("request serialises");
        assert!(value.get("model").is_none());
        assert_eq!(value["state"]["typed_tag"], json!("brkout"));
    }
}
