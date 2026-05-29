//! ENG-01a — Dignitas Score as a living artifact.
//!
//! Daily snapshot persistence, 7-day delta, 90-day sparkline panel,
//! and a transparency page. Ungameable by design: frequency, raw P&L,
//! and win rate are never inputs.
//!
//! Submodule layout:
//!   types.rs    — domain structs + API wire types (ENG-01a)
//!   inputs.rs   — 5 pure input-computation functions (ENG-01a)
//!   config.rs   — load DignitasWeights from dignitas_config table (ENG-01a)
//!   snapshot.rs — orchestrator: assemble inputs → formula → upsert (ENG-01a)
//!   schedule.rs — daily UTC 00:30 scheduler (ENG-01a)
//!   handles/    — handle claim/release/visibility + public profile (ENG-01b)
//!   streak.rs   — days-since-Concerning-flag counter + longest_ever (ENG-01c)

// @anchor exchange:router:mod
// @tags api

pub mod config;
pub mod handles;
pub mod inputs;
pub mod schedule;
pub mod snapshot;
pub mod streak;
pub mod types;

pub use types::*;
