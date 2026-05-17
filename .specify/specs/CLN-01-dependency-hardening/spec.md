# Specification: Patch All HIGH CVEs and Update Dependencies

**Spec ID:** CLN-01-dependency-hardening
**Date:** 2026-05-15
**Status:** Implemented
**Class:** Infrastructure / Security
**Priority:** P0 — 8 HIGH-severity vulnerabilities, active Chrome Web Store submission
**Depends on:** None (first in series)
**Series:** CLN-01 through CLN-09 (Phase 1 — Open-Source Readiness Cleanup)

---

## Problem Statement

The DevSecOps audit (`DEVSECOPS_AUDIT.md`, 2026-04-03) identified 12 Rust security advisories (8 HIGH, 4 MEDIUM) and 9 unmaintained crate warnings in `testudo-exchange`, plus 3 Node.js vulnerabilities (2 HIGH) in `testudo-extension`. Running `cargo audit` today confirms the advisories are still open.

Key vulnerabilities:
- `alloy-dyn-abi 0.7.7` — **CVSS 7.5** DoS via `TypedData` hashing (requires alloy ecosystem upgrade)
- `idna 0.4.0` — Punycode bypass (transitive via `url`/`reqwest`)
- `protobuf 2.28.0` — Crash via uncontrolled recursion
- `rsa 0.9.10` — **Marvin Attack**, no fix available (medium, but notable)
- `backoff`, `derivative`, `instant`, `paste`, `proc-macro-error`, `rustls-pemfile`, `lru` — unmaintained

Node.js:
- `picomatch 4.0.0-4.0.3` — **HIGH** method injection
- `rollup 4.0.0-4.58.0` — **HIGH** arbitrary file write via path traversal
- `esbuild <=0.24.2` — **MODERATE** dev server data leak

This is the highest-impact, lowest-effort item in Phase 1. Most CVEs patch with a single command. The alloy ecosystem upgrade is the only multi-step change.

---

## User Stories

- **As a user running the extension**, I want to know the dependencies powering my trade execution aren't vulnerable to known CVEs.
- **As the developer**, I want `cargo audit` and `npm audit` to return clean results before any public release.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `cargo update` patches all fixable Rust CVEs (bytes, aws-lc-sys, rustls-webpki, time, tracing-subscriber, rkyv, tokio) | High | testudo-exchange |
| FR-2 | Upgrade alloy ecosystem to >=0.8.26 to fix `alloy-dyn-abi` CVSS 7.5 | High | testudo-exchange |
| FR-3 | Pin or replace unmaintained crates: `backoff`, `derivative`, `paste`, `proc-macro-error`, `rustls-pemfile` | Medium | testudo-exchange |
| FR-4 | `npm audit fix` patches `picomatch` and `rollup` in testudo-extension | High | testudo-extension |
| FR-5 | Evaluate esbuild upgrade to >=0.28.0 (breaking change — verify build pipeline) | Medium | testudo-extension |
| FR-6 | `cargo clippy --all-targets` and `cargo test` must pass after all upgrades | High | testudo-exchange |
| FR-7 | `bun run build` and `bun run test` must pass after npm patches | High | testudo-extension |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `cargo update` + `npm audit fix` — bump transitive deps only | CVEs that patch with semver-compatible bumps are resolved |
| CP-2 | Upgrade alloy from 0.1.4 to >=0.8.26 | `alloy-dyn-abi` 7.5 fixed; Hyperliquid SDK compatibility verified |
| CP-3 | Replace/remove unmaintained crate usages | `cargo audit` shows zero unmaintained warnings |
| CP-4 | Full verification: clippy + tests + build | CI-equivalent green across all subprojects |

### Dependency Changes

#### Rust: `cargo update` (transitive patches)

```bash
cd testudo-exchange && cargo update
```

Expected to patch:
- `bytes` 1.7.1 → >=1.11.1 (integer overflow fix)
- `aws-lc-sys` 0.38.0 → >=0.39.0 (CRL/Name Constraints fixes)
- `rustls-webpki` 0.103.9 → >=0.103.10
- `time` 0.3.36 → >=0.3.47
- `tracing-subscriber` 0.3.18 → >=0.3.20
- `rkyv` 0.7.45 → >=0.7.46
- `tokio` 1.40.0 → latest 1.x

#### Rust: alloy ecosystem upgrade

```toml
# testudo-exchange/Cargo.toml [workspace.dependencies]
- alloy = "0.1.4"
+ alloy = { version = "0.8", features = ["..."] }  # pin after verifying Hyperliquid SDK compat
```

**Risk:** `hyperliquid-sdk-rs` depends on alloy 0.1.4. If the SDK hasn't been updated to support alloy 0.8+, we must:
1. Fork `hyperliquid-sdk-rs` and upgrade its alloy dependency, OR
2. Use a `[patch]` section to override alloy versions, OR
3. Accept the risk and document the exception

#### Rust: unmaintained crate replacements

| Crate | Action |
|-------|--------|
| `backoff` 0.4.0 | Replace with `backon` or manual exponential-backoff impl |
| `derivative` 2.2.0 | Replace with `educe` or `derive_more` |
| `paste` 1.0.15 | Replace with manual impl or `concat_idents` where possible |
| `proc-macro-error` 1.0.4 | Replace with `proc-macro-error2` or `manyhow` |
| `rustls-pemfile` 1.0.4 | Already partially on 2.2.0 — upgrade remaining usage |
| `instant` 0.1.13 | Replace with `std::time::Instant` |
| `lru` 0.12.5 | Pin and monitor; `IterMut` UB is test-only risk |

#### Node.js: npm audit fix

```bash
cd testudo-extension && npm audit fix
```

Patches `picomatch` and `rollup`. For `esbuild`:
```bash
npm install esbuild@^0.28.0
# Verify: bun run build && bun run test
```

### Files

- `testudo-exchange/Cargo.toml` — alloy version bump, potentially add `[patch]`
- `testudo-exchange/Cargo.lock` — regenerated by `cargo update`
- `testudo-exchange/crates/router/Cargo.toml` — if alloy features change
- `testudo-extension/package.json` — esbuild version bump
- `testudo-extension/package-lock.json` — regenerated

### Dependencies Added

- `backon` or manual backoff — replaces `backoff`
- `educe` or `derive_more` — replaces `derivative`
- `proc-macro-error2` or `manyhow` — replaces `proc-macro-error`

---

## Acceptance Criteria

- [x] `cargo audit` shows zero HIGH or CRITICAL advisories
- [x] `cargo audit` shows zero unmaintained warnings (or each remaining one has a documented exception)
- [x] `npm audit` shows zero HIGH or CRITICAL advisories in testudo-extension
- [x] `cargo clippy --all-targets` passes with no new warnings
- [x] `cargo test` passes (excluding the 2 pre-existing failures fixed in CLN-02)
- [x] `cd testudo-extension && bun run build` passes
- [x] Hyperliquid order placement, fill detection, and cancel tested on dev
- [x] CI pipeline (`ci.yml`) passes on PR

---

## Risks

1. **Alloy ecosystem upgrade breaks Hyperliquid SDK.** `hyperliquid-sdk-rs@0.1.2` pins alloy 0.1.4. Mitigation: attempt upgrade first; if SDK incompatible, fork SDK and upgrade, or document exception with compensating controls.
2. **Unmaintained crate replacement is non-trivial.** Some crates (derivative, paste) are used in macro-heavy code. Mitigation: prioritize security CVEs first; unmaintained warnings are secondary. Document any that remain.
3. **esbuild 0.28 breaking change.** Extension build may fail. Mitigation: test immediately after install; roll back if pipe breaks and document.

---

## Completion Signal

This spec is complete when:
1. `cargo audit` reports zero HIGH/CRITICAL advisories
2. `npm audit` reports zero HIGH/CRITICAL advisories
3. All verification commands pass
4. Code committed to master
