# Specification: Sheaf Engine — Laplacian Computations Cleanup

**Spec ID:** SHEAF-01-laplacian-cleanup
**Date:** 2026-05-29
**Status:** Draft
**Class:** Infrastructure / Refactor
**Priority:** P1 — nuclear review flagged 3 blockers and 2 serious issues; each is a regression that makes the codebase harder to maintain, reuse, or configure
**Depends on:** None (scoped to existing `sheaf-engine/` crate)
**Series:** SHEAF-01 (Sheaf Engine Quality) — distinct from STRAT (strategy verification) and AGENT (agent infrastructure)

---

## Problem Statement

A thermo-nuclear code quality review of commits `e4cd324..HEAD` on `master` (6 Rust files in `sheaf-engine/`, 317 insertions, 132 deletions) identified 3 blockers and 2 serious issues. The changes introduced an `Arc<RwLock<>>` wrapper for thread safety, added `debug_assert!` invariants, renamed `eigengap` → `eigen_gap`, and reformatted tests. These are directionally correct, but the implementation left behind:

1. **Triple-copy `now_ns()`:** Three identical 4-line timestamp functions exist in `graph.rs:430` (private), `laplacian.rs:202` (module-private), and `signals.rs:280` (module-private). `SheafGraph` already owns the canonical implementation — the other two modules copy-pasted it. This is a canonical-helper-duplication violation (rule 6). The diff touched `laplacian.rs` and `signals.rs` without cleaning this up.

2. **New `is_symmetric` introduces clippy warnings:** The `is_symmetric` function added in `laplacian.rs:147–148` uses `for i in 0..n` range loops that clippy flags as `needless_range_loop`. New code should not introduce new diagnostics. This is a quality regression (rule 4).

3. **Guardrail message mismatch in `compute_laplacian`:** `debug_assert!(node_count <= 256, "graph node count {node_count} exceeds design target of 30")` — the guard allows 256 but the message says "target of 30." The check fires at 8.5× the stated design point, creating confusion about what is being guarded (rule 4).

4. **Magic number bypasses `GraphConfig` in `extract_arbitrage_signals`:** `signals.rs:76` uses hardcoded `spread_bps < 5.0` as the arbitrage threshold, but `GraphConfig.arbitrage_signal_threshold_bps` (default `5.0`) exists specifically for this purpose. The function receives `&SheafGraph` which carries `graph.config`. Changing the config value silently has no effect on signal extraction (rule 6).

5. **`connected_components: 1` is a silent lie:** `service.rs:131` hardcodes `connected_components: 1` in `GraphState` without computing it. If the graph is actually disconnected, the consuming agent receives false information. The field should at minimum carry a `TODO` marker (rule 4).

---

## User Stories

- **As a sheaf-engine maintainer**, I want a single canonical `now_ns()` helper so that timestamp logic doesn't silently diverge across modules.
- **As a sheaf-engine maintainer**, I want `cargo clippy --all-targets` to pass with zero warnings on my changes so that new code meets the baseline quality bar.
- **As a Testudo operator**, I want `GraphConfig.arbitrage_signal_threshold_bps` to actually control the arbitrage signal threshold so that configuration changes have the intended effect.
- **As a sheaf-engine consumer** (agent harness), I want `GraphState.connected_components` to be honest — either computed correctly or explicitly marked as unimplemented — so that I don't make decisions on false data.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Extract `now_ns()` to a canonical `pub(crate)` location, delete the two duplicate copies in `laplacian.rs` and `signals.rs`, and rewire all call sites. | High | `sheaf-engine/src/` |
| FR-2 | Rewrite `is_symmetric` in `laplacian.rs` using iterators (`iter().enumerate()`) to eliminate `needless_range_loop` clippy warnings. | High | `laplacian.rs` |
| FR-3 | Fix the `debug_assert!` message in `compute_laplacian` to match the guard value (`node_count <= 256`, not "design target of 30"). Remove or tighten the guard if the design target is genuinely 30. | High | `laplacian.rs` |
| FR-4 | Wire `graph.config.arbitrage_signal_threshold_bps` into `extract_arbitrage_signals`, replacing the hardcoded `5.0`. The `extract_signals` function signature must pass the config value (either via `&SheafGraph` or an explicit parameter). | High | `signals.rs` |
| FR-5 | Replace `connected_components: 1` in `service.rs:131` with `0` and append a `// TODO: compute connected components from graph` comment. | Medium | `service.rs` |
| FR-6 | Run `cargo clippy --all-targets && cargo test` — must exit 0 with **zero warnings** (suppress pre-existing warnings in untouched files with `#[allow(...)]` only if necessary). | High | `sheaf-engine/` |

---

## Technical Implementation

### Vertical Checkpoints

All changes are small, mechanical, and independent of each other. They can be batched into two checkpoints or done in a single pass.

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | FR-1 (canonical `now_ns`), FR-2 (`is_symmetric` iterator rewrite), FR-3 (guard message fix), FR-5 (`connected_components` TODO) | `cargo clippy` zero warnings, `cargo test` 12/12 pass |
| CP-2 | FR-4 (wire `arbitrage_signal_threshold_bps` into signal extraction), FR-6 (final clippy + test gate) | `cargo clippy --all-targets && cargo test`, verify config threshold actually controls signal emission |

### FR-1: Canonical `now_ns()`

Two options — prefer option A for minimal structural change:

**Option A — promote `SheafGraph::now_ns()` to `pub(crate)`:**

```rust
// graph.rs — change from `fn now_ns()` to:
pub(crate) fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64
}
```

Then in `laplacian.rs` and `signals.rs`, delete the local `fn now_ns()` and use `crate::graph::now_ns()` instead. Update all call sites: `now_ns()` → `crate::graph::now_ns()`.

**Option B — extract to a new `time.rs` module:**

```rust
// src/time.rs (new file)
/// Current wall-clock time in nanoseconds since Unix epoch.
///
/// Falls back to 0 if the system clock is before 1970 (should never happen).
pub(crate) fn now_ns() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as i64
}
```

Then add `pub mod time;` to `lib.rs` and update all call sites. This is cleaner long-term if more timestamp utilities are expected.

**Decision: Option A** — the simplest path that eliminates duplication. `SheafGraph` already owns the concept; promoting it to `pub(crate)` adds zero new modules and zero new concepts to the crate.

### FR-2: `is_symmetric` Iterator Rewrite

Current code (clippy warnings at lines 147–148):

```rust
fn is_symmetric(matrix: &[Vec<f64>], n: usize) -> bool {
    for i in 0..n {
        for j in (i + 1)..n {
            if (matrix[i][j] - matrix[j][i]).abs() > 1e-10 {
                return false;
            }
        }
    }
    true
}
```

Rewrite using iterators:

```rust
fn is_symmetric(matrix: &[Vec<f64>], n: usize) -> bool {
    matrix.iter().enumerate().take(n).all(|(i, row)| {
        row.iter().enumerate().skip(i + 1).take(n.saturating_sub(i + 1)).all(|(j, &val)| {
            (val - matrix[j][i]).abs() <= 1e-10
        })
    })
}
```

Alternatively, a slightly more readable two-loop version that still satisfies clippy:

```rust
fn is_symmetric(matrix: &[Vec<f64>], n: usize) -> bool {
    for (i, row) in matrix.iter().enumerate().take(n) {
        for (j, &val) in row.iter().enumerate().skip(i + 1).take(n.saturating_sub(i + 1)) {
            if (val - matrix[j][i]).abs() > 1e-10 {
                return false;
            }
        }
    }
    true
}
```

**Decision: Two-loop version** — slightly more lines but far more readable for `O(n²)` symmetry checks where early-exit semantics (`return false`) are clearer than nested `all()` closures.

### FR-3: Guard Message Fix

Two paths:

**Path A — fix the message to match the guard:**
```rust
debug_assert!(
    node_count <= 256,
    "graph node count {node_count} exceeds internal guard of 256; \
     graph is designed for 15-30 nodes"
);
```

**Path B — lower the guard to match the design target:**
```rust
debug_assert!(
    node_count <= 32,
    "graph node count {node_count} exceeds design target of 30"
);
```

**Decision: Path A** — the guard at 256 is a genuine safety net (O(n²) eigenvalue decomposition that would blow up at very large n), not a design enforcement. Keep the guard, fix the message. A 30-node guard would fire spuriously in edge cases with more watch targets.

### FR-4: Wire Config Threshold into Signal Extraction

Current hardcoded threshold in `signals.rs:76`:

```rust
if spread_bps < 5.0 {
    continue;
}
```

The function `extract_arbitrage_signals` receives `graph: &SheafGraph` which carries `graph.config`. The fix is to read the threshold from the config:

```rust
// In extract_arbitrage_signals:
let threshold = graph.config.arbitrage_signal_threshold_bps;

// ... later:
if spread_bps < threshold {
    continue;
}
```

`GraphConfig` fields are all private, so this requires either:
- Making `arbitrage_signal_threshold_bps` `pub` (simple)
- Adding a getter `pub fn arbitrage_signal_threshold_bps(&self) -> f64` (cleaner)
- Making all of `GraphConfig` fields `pub(crate)` (if all signal extraction code needs config access)

**Decision: Add a getter** — `pub fn arbitrage_signal_threshold_bps(&self) -> f64` on `GraphConfig`. This keeps the field private while allowing the one caller that needs it. If more config fields are needed later, the pattern extends naturally.

### FR-5: `connected_components` TODO

```rust
// Before:
connected_components: 1,

// After:
connected_components: 0, // TODO: compute connected components from graph
```

Setting to `0` makes it explicit that this metric is not computed. A consumer checking `is_connected == true` but seeing `connected_components == 0` will understand the metric is unavailable.

### Files

- `sheaf-engine/src/graph.rs` — promote `now_ns()` to `pub(crate)`; add `arbitrage_signal_threshold_bps()` getter
- `sheaf-engine/src/laplacian.rs` — delete duplicate `now_ns()`, rewire call sites to `crate::graph::now_ns()`; rewrite `is_symmetric` with iterators; fix guard message
- `sheaf-engine/src/signals.rs` — delete duplicate `now_ns()`, rewire call sites; wire config threshold into `extract_arbitrage_signals`
- `sheaf-engine/src/service.rs` — replace `connected_components: 1` with `0` + TODO comment
- `sheaf-engine/src/lib.rs` — no changes (Option A needs no new module)

### Dependencies Added

None. All changes are internal to `sheaf-engine/`.

---

## Acceptance Criteria

- [ ] All three `now_ns()` duplicates collapsed into one canonical `pub(crate)` function in `graph.rs`; `laplacian.rs` and `signals.rs` call sites updated
- [ ] `is_symmetric` rewritten with iterators; `cargo clippy` produces **zero** `needless_range_loop` warnings
- [ ] Guard message in `compute_laplacian` matches the guard value (256)
- [ ] `extract_arbitrage_signals` reads threshold from `graph.config.arbitrage_signal_threshold_bps` instead of hardcoded `5.0`
- [ ] `connected_components` set to `0` with a `// TODO` comment in `service.rs`
- [ ] `cargo clippy --all-targets` exits 0 with zero warnings on files touched by this diff (pre-existing warnings in `align.rs` are acceptable but should be suppressed or fixed opportunistically)
- [ ] `cargo test` — all 12 existing tests pass

---

## Risks

1. **`now_ns()` call site breakage** — changing from local `now_ns()` to `crate::graph::now_ns()` is mechanical but there are ~5 call sites across `laplacian.rs` and `signals.rs`. Mitigation: `cargo test && cargo check` catches all of them.
2. **Config threshold behavior change** — if someone was relying on the hardcoded `5.0` threshold and has a `GraphConfig` with a different `arbitrage_signal_threshold_bps`, the filter behavior changes. Mitigation: this is the intended behavior; the hardcoded value was the bug.
3. **Clippy pre-existing warnings** — `align.rs` has 3 pre-existing clippy warnings (`unnecessary_map_or`, `filter_map_identity` ×2) that are not in scope for this spec. Decision: address them opportunistically if trivial, otherwise leave them. They are pre-existing, not regressions.

---

## Completion Signal

This spec is complete when:
1. `cargo clippy --all-targets` produces zero warnings on files modified by this spec
2. `cargo test` — 12/12 pass
3. All acceptance criteria checked off
4. Code committed to master with message `fix: sheaf-engine laplacian cleanup — canonical now_ns, clippy clean, config threshold`
