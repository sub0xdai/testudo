# Specification: Standardize on `tracing` — Remove All `eprintln!`

**Spec ID:** CLN-06-tracing-standardization
**Date:** 2026-05-15
**Status:** Draft
**Class:** Refactor / Observability
**Priority:** P1 — mixed logging (eprintln! + tracing) violates observability standards and pollutes stderr
**Depends on:** CLN-01, CLN-02
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

The TigerBeetle comparison audit flagged observability as a weakness:

> *"eprintln! used in production code paths instead of structured tracing — mixing stdout/stderr with proper observability."*

A scan of non-test Rust code found **22 instances** of `eprintln!` across:
- `engine/src/engine/engine.rs` — 5 uses (deprecation warning, orderbook not found, etc.)
- `engine/src/engine/ws_stream.rs` — 1 use
- `engine/src/order.rs` — 6 uses (missing pubsub_id warnings)
- `engine/src/user.rs` — 1 use
- `ws-stream/src/pg_ws_manager.rs` — 2 uses (invalid subscription format)
- `ws-stream/src/main.rs` — 4 uses (TCP_NODELAY, listen/unlisten failures, notification errors)
- `sqlx_postgres/src/repositories/api_keys.rs` — 1 use (DB unavailable in test)
- `router/src/main.rs` — 2 uses (config validation, migration failure)

The project already uses the `tracing` crate with `tracing-subscriber` (configured with JSON and env-filter features). The `tracing` and `tracing-subscriber` crates are in `Cargo.toml` workspace dependencies. Most of the codebase uses `tracing::info!`, `tracing::error!`, `tracing::warn!` — but these 22 `eprintln!` calls bypass the structured logging pipeline entirely.

Additionally, `eprintln!` writes to stderr unconditionally, polluting the terminal in local dev and failing to appear in structured log aggregators (Grafana Loki, Cloud Logging) in production.

---

## User Stories

- **As an operator**, I want all log output to go through the `tracing` pipeline, so that errors appear in Grafana dashboards and structured log queries work.
- **As a developer**, I want `eprintln!` to be a non-idiom in this codebase, so that grep for `eprintln` finds zero matches and new contributors use `tracing` by default.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace all `eprintln!` in production code with appropriate `tracing` macros (`error!`, `warn!`, `info!`) | High | All |
| FR-2 | Use structured fields (`key = value`) in tracing calls where available | Medium | All |
| FR-3 | Test-only `eprintln!` (`api_keys.rs:736`) — keep as-is or convert if trivial | Low | sqlx_postgres |
| FR-4 | Add `#![deny(clippy::print_stderr)]` to engine and ws-stream crates (with test exemptions) | Medium | engine/, ws-stream/ |
| FR-5 | `cargo clippy --all-targets` passes with zero `print_stderr` warnings in non-test code | High | All |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Replace `eprintln!` in `engine/` (engine.rs, order.rs, user.rs, ws_stream.rs) — 13 instances | `cargo check` in engine crate passes |
| CP-2 | Replace `eprintln!` in `ws-stream/` (main.rs, pg_ws_manager.rs) — 6 instances | `cargo check` in ws-stream crate passes |
| CP-3 | Replace `eprintln!` in `router/src/main.rs` — 2 instances | Router compiles and starts |
| CP-4 | Add clippy lint to deny `eprintln!` / `print_stderr` in non-test code | `cargo clippy` enforces |

### Migration Pattern

**Before (engine.rs):**
```rust
eprintln!("No matching orderbook found for market: {}", market);
eprintln!("[DEPRECATED] Legacy Engine::create_order_pg() called for user {}. Orders must use Decision Loop.", input_order.user_id);
eprintln!("CreateOrder missing pubsub_id, cannot respond");
```

**After:**
```rust
tracing::warn!(market = %market, "No matching orderbook found");
tracing::error!(user_id = %input_order.user_id, "Deprecated create_order_pg called — orders must use Decision Loop");
tracing::warn!("CreateOrder missing pubsub_id, cannot respond");
```

### Severity Mapping

| Current `eprintln!` context | `tracing` level | Notes |
|---|---|---|
| Deprecation warnings (`[DEPRECATED]`) | `error!` | Should not happen in prod — high severity |
| Missing pubsub_id (can't respond) | `warn!` | Operational issue but not fatal |
| Orderbook not found | `warn!` | May be expected during market transitions |
| Config validation failure | `error!` | Fatal — process may exit |
| Migration failure | `error!` | Fatal |
| TCP_NODELAY failure | `warn!` | Non-fatal, performance impact only |
| Notification receive error | `error!` | May indicate channel issues |
| Invalid subscription format | `warn!` | Client error, not server |
| DB not available (test) | Keep `eprintln!` | Test infrastructure — not in production path |

### Structured Field Convention

Use `field = %value` for Display-format values (strings, IDs):
```rust
tracing::warn!(market = %market, order_id = %order_id, "Orderbook not found");
tracing::error!(user_id = %user_id, "Deprecated code path invoked");
```

Use `field = ?value` for Debug-format values:
```rust
tracing::error!(error = ?err, "Migration failed");
```

### Clippy Lint Enforcement

Add to `crates/engine/src/lib.rs` and `crates/ws-stream/src/main.rs`:
```rust
#![cfg_attr(not(test), deny(clippy::print_stderr))]
```

This allows `eprintln!` in tests but forbids it in production code.

### Files

- `testudo-exchange/crates/engine/src/engine/engine.rs` — 5 `eprintln!` → `tracing`
- `testudo-exchange/crates/engine/src/engine/ws_stream.rs` — 1 `eprintln!` → `tracing`
- `testudo-exchange/crates/engine/src/order.rs` — 6 `eprintln!` → `tracing`
- `testudo-exchange/crates/engine/src/user.rs` — 1 `eprintln!` → `tracing`
- `testudo-exchange/crates/ws-stream/src/main.rs` — 4 `eprintln!` → `tracing`
- `testudo-exchange/crates/ws-stream/src/pg_ws_manager.rs` — 2 `eprintln!` → `tracing`
- `testudo-exchange/crates/router/src/main.rs` — 2 `eprintln!` → `tracing`
- `testudo-exchange/crates/sqlx_postgres/src/repositories/api_keys.rs` — 1 `eprintln!` (test-only, keep)

### Dependencies Added

None — `tracing` is already a workspace dependency.

---

## Acceptance Criteria

- [ ] Zero `eprintln!` calls in production Rust code (`engine/`, `ws-stream/`, `router/`)
- [ ] All 22 instances replaced with appropriate `tracing::error!`, `tracing::warn!`, or `tracing::info!`
- [ ] Structured fields used where values are available (`market = %market`, `user_id = %user_id`)
- [ ] `cargo check` passes in all affected crates
- [ ] `cargo clippy --all-targets` passes with no `print_stderr` warnings in non-test code
- [ ] `cargo test` passes

---

## Risks

1. **Missing tracing subscriber in binary entry points.** If `main.rs` in ws-stream or router doesn't initialize `tracing-subscriber`, the `tracing` macros produce no output (silent failure). Mitigation: verify that `tracing_subscriber::fmt::init()` or equivalent is called early in every `main.rs` before replacing `eprintln!`.
2. **Test code already uses `eprintln!` for debug output.** We allow this — the clippy lint exempts test code. Only production paths are in scope.

---

## Completion Signal

This spec is complete when:
1. `rg "eprintln!\(" testudo-exchange/crates/ --include='*.rs' | grep -v 'test' | grep -v '#\[cfg'` returns zero matches
2. `cargo clippy --all-targets` passes
3. `cargo test` passes
4. Code committed to master
