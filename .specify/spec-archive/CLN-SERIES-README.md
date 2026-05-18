# CLN Series — Phase 1: Open-Source Readiness Cleanup

**Series ID:** CLN-01 through CLN-10
**Date:** 2026-05-15
**Status:** Draft — All 10 specs created, none implemented
**Goal:** Prepare Testudo for public open-source release by fixing security vulnerabilities, test regressions, code quality issues, and safety gaps identified in audits.

**Audit coverage:** Addresses 100% of the 5 "Highest-Impact Changes" from the TigerBeetle comparison audit, plus the DevSecOps audit's clippy warnings and the `check_and_lock_funds` atomicity gap.

---

## Spec Inventory

| # | Spec | Priority | Est. Effort | Depends On | Status |
|---|------|----------|-------------|------------|--------|
| CLN-01 | [Dependency Hardening](./CLN-01-dependency-hardening/spec.md) | P0 | 2-4h | — | Draft |
| CLN-02 | [Fix Failing Tests](./CLN-02-failing-test-fix/spec.md) | P0 | 2-4h | CLN-01 | Draft |
| CLN-03 | [Typed Engine Errors](./CLN-03-typed-engine-errors/spec.md) | P1 | 4-6h | CLN-01, CLN-02 | Draft |
| CLN-04 | [Remove unwrap() from Prod Paths](./CLN-04-remove-unwrap/spec.md) | P1 | 2-3h | CLN-01, CLN-02, CLN-03 | Draft |
| CLN-05 | [Split shadow/actor.rs](./CLN-05-split-shadow-actor/spec.md) | P1 | 4-6h | CLN-01..CLN-04 | Draft |
| CLN-06 | [Tracing Standardization](./CLN-06-tracing-standardization/spec.md) | P1 | 2-3h | CLN-01, CLN-02 | Draft |
| CLN-07 | [Delete Deprecated Code](./CLN-07-delete-deprecated-code/spec.md) | P1 | 1-2h | CLN-01 | Draft |
| CLN-08 | [Secret History Scan](./CLN-08-secret-history-scan/spec.md) | P0 | 1-2h | — (parallel-safe) | Draft |
| CLN-09 | [Assertion Discipline](./CLN-09-assertion-discipline/spec.md) | P1 | 3-4h | CLN-01..CLN-04 | Draft |
| CLN-10 | [Clippy + Balance Atomicity](./CLN-10-clippy-and-balance-atomicity/spec.md) | P1 | 1-2h | CLN-01, CLN-03 | Draft |

---

## Execution Order

```
                    +--------------------------+
                    |     CLN-01 Deps           |
                    |     (cargo update +       |
                    |      npm audit fix)       |
                    +------------+-------------+
                                 |
              +------------------+------------------+
              |                  |                  |
              v                  v                  v
    +-----------------+  +------------+  +------------------+
    |  CLN-02 Tests   |  | CLN-06     |  |  CLN-08          |
    |  (fix 2 failing)|  | (tracing)  |  |  (secrets scan)  |
    +--------+--------+  +-----+------+  +------------------+
             |                 |           (parallel - no deps)
    +--------v--------+       |
    |  CLN-03 Errors  |<------+
    |  (typed enums)  |
    +--------+--------+
             |
    +--------v--------+
    |  CLN-04 unwrap  |
    |  (remove from   |
    |   production)   |
    +--------+--------+
             |
    +--------v--------+     +------------------+
    |  CLN-05 Actor   |     |  CLN-07          |
    |  (split 2K line)|     |  (deprecated)    |
    +--------+--------+     +------------------+
             |                (can run after CLN-01)
    +--------v--------+
    |  CLN-09 Assert  |
    |  (add to engine)|
    +--------+--------+
             |
    +--------v--------+
    |  CLN-10 Clippy  |
    |  + Balance Atom |
    +-----------------+
```

---

## TigerBeetle Audit Coverage

All 5 "Highest-Impact Changes" from the audit are addressed:

| # | Audit Finding | CLN Spec | Status |
|---|---------------|----------|--------|
| 1 | Assertion discipline (0 → >=10) | CLN-09 | ✅ |
| 2 | Split shadow/actor.rs (2,029 → <=300) | CLN-05 | ✅ |
| 3 | Typed engine errors (string → enum) | CLN-03 | ✅ |
| 4 | Remove unwrap() from production | CLN-04 | ✅ |
| 5a | Remove deprecated code | CLN-07 | ✅ |
| 5b | Standardize on tracing (remove eprintln!) | CLN-06 | ✅ |
| - | Fix 2 failing tests | CLN-02 | ✅ |
| - | error.rs commented out → real EngineError enum | CLN-03 | ✅ |
| - | init_engine DB .unwrap() | CLN-04 | ✅ |
| - | check_and_lock_funds atomicity gaps | CLN-10 | ✅ |
| - | 3 clippy warnings | CLN-10 | ✅ |
| - | 8 HIGH Rust CVEs | CLN-01 | ✅ |
| - | 2 HIGH Node.js CVEs | CLN-01 | ✅ |
| - | Secrets in git history | CLN-08 | ✅ |

**Not covered — architectural decisions, not bugs:**
- No batching (each order processed individually) — design tradeoff, Phase 2+
- `Arc<Mutex<Engine>>` serialization — intentional, Phase 2+
- PG as message queue — intentional architecture decision
- No DST / simulation testing — Phase 2
- `engine.rs` 489 lines (exceeds 70-line TB limit) — CLN-05 splits actor.rs only; splitting engine.rs is lower ROI
- `main.rs` 1,421 lines — common Rust pattern, not a bug

---

## Verification Gate

Before marking the series complete, run:

```bash
# Backend
cd testudo-exchange && cargo clippy --all-targets -- -D warnings && cargo test
# Must show: 0 clippy warnings, 0 test failures

# Extension
cd testudo-extension && bun run build
# Must show: clean build

# Security audit
cargo audit --file testudo-exchange/Cargo.lock
# Must show: 0 HIGH/CRITICAL advisories

cd testudo-extension && npm audit
# Must show: 0 HIGH/CRITICAL advisories
```

---

## What's NOT in Scope (Phase 2 and beyond)

- PostgreSQL 12 -> 16 upgrade (scheduled maintenance window)
- UUID v7 migration (revisit at 10M+ rows)
- DST / simulation testing (VOPR-style)
- CI pre-commit gitleaks hook
- Documentation improvements (README, CONTRIBUTING.md, ARCHITECTURE.md)
- License selection and application

---

## Summary

| Metric | Before (Current) | After (Target) |
|--------|-----------------|----------------|
| HIGH Rust CVEs | 8 | 0 |
| HIGH Node.js CVEs | 2 | 0 |
| Failing tests | 2 | 0 |
| String errors in engine | All public fns | 0 (typed EngineError) |
| `unwrap()` in production | 3 instances | 0 |
| `eprintln!` in production | 22 instances | 0 |
| `shadow/actor.rs` lines | 2,029 | <=300 |
| Assertions in engine.rs | 0 | >=10 |
| Deprecated code | `create_order_pg` + others | 0 |
| Clippy warnings | 3 | 0 (deny-all enforced) |
| Balance atomicity guards | 0 | 3 `debug_assert!` in `check_and_lock_funds` |
| Secrets in git history | Unknown | Verified clean |
