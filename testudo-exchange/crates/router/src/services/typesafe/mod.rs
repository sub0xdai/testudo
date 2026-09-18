//! TS-01 — TypeSafe System One (Jev) integration.
//!
//! Jev returns typed judgements, not prose. Nothing in this module generates
//! text, and nothing in it participates in sizing or order parameters: a
//! judgement labels a trade, and the label is what feeds calibration once the
//! trade has a realised outcome.
//!
//! # Presence is the feature flag
//!
//! [`AppState::typesafe_client`](crate::types::app::AppState) is an `Option`.
//! `None` means the integration is off, or no credential was provisioned.
//! There is no `enabled` boolean anywhere below this point, so a caller cannot
//! forget to check one. Every call site degrades to "unavailable" silently.
//!
//! Design, scope, and the decisions behind the retry split:
//! `docs/plans/typesafe-jev-sniper-integration.md`.

// @anchor exchange:router:typesafe
// @tags api

pub mod attribution;
pub mod client;
pub mod service;
pub mod types;

pub use attribution::{
    attribute_note, decide_attribution, spawn_note_attribution, Attribution, AttributionAudit,
    AttributionStatus, NoteAttribution, NoteContext, EVENT_TYPE, EXIT_DISCIPLINE_LEVELS,
};
pub use client::{HttpSystemOneClient, MockSystemOneClient, SystemOneClient};
pub use service::{
    resolve_setup_tag, TagCandidate, TagResolution, TagResolutionInput, SETUP_TAG_QUESTION_ID,
};
pub use types::{
    backoff_delay, Answer, CallPolicy, ChoiceCriteria, NoulCriteria, Question, ScoreCriteria,
    State, SystemOneRequest, SystemOneResponse, TypeSafeError, Usage, API_KEY_ENV,
    DEFAULT_BASE_URL, MODEL_ALIAS, MODEL_PINNED,
};
