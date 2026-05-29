//! RSK-03 — AI Trade Coach
//!
//! Weekly behavioral insights pipeline:
//!   baseline + week stats → pattern detectors → CoachDigest
//!     → Narrator (LLM) → citation Validator → persisted CoachReport.
//!
//! All submodules are stubbed in T2; logic lands in T3+.

// @anchor exchange:router:mod
// @tags api

pub mod digest;
pub mod narrator;
pub mod patterns;
pub mod schedule;
pub mod service;
pub mod types;
pub mod validator;

pub use service::CoachService;
pub use types::*;
