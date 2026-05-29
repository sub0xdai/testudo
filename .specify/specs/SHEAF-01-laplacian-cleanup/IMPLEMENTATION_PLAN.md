# SHEAF-01-laplacian-cleanup — Implementation Plan

## Current State Summary

The sheaf-engine crate has 5 code quality issues identified in a nuclear review of commits `e4cd324..HEAD`:
1. **Triple `now_ns()` duplication** — identical 4-line timestamp functions in `graph.rs` (private), `laplacian.rs` (module-private), and `signals.rs` (module-private). Only `graph.rs` should own it.
2. **`is_symmetric` clippy warnings** — range loops at `laplacian.rs:149` flagged as `needless_range_loop`.
3. **Guard message mismatch** — `debug_assert!` at `laplacian.rs:67` uses `node_count <= 256` but message says "design target of 30".
4. **Hardcoded arbitrage threshold** — `signals.rs:76` uses literal `5.0` instead of `graph.config.arbitrage_signal_threshold_bps`.
5. **`connected_components: 1` is a lie** — `service.rs:131` hardcodes `1` without computation.

All changes are mechanical, internal to `sheaf-engine/`, with no new dependencies. 12 existing tests must pass, zero new clippy warnings on touched files.

## Checkpoints

### CP-1: Canonical now_ns, clippy clean, guard fix, connected_components TODO ✅
- Completed 2026-05-29 by /skill:vox build. 12/12 tests pass, 0 needless_range_loop warnings.
- **Touches**: `sheaf-engine/src/graph.rs`, `sheaf-engine/src/laplacian.rs`, `sheaf-engine/src/signals.rs`, `sheaf-engine/src/service.rs`
- **Tasks**:
  1. `graph.rs`: change `fn now_ns()` → `pub(crate) fn now_ns()`
  2. `graph.rs`: add `pub(crate) fn config(&self) -> &GraphConfig` accessor
  3. `laplacian.rs`: delete duplicate `fn now_ns()` (line 205); replace `now_ns()` calls with `crate::graph::now_ns()`
  4. `laplacian.rs`: rewrite `is_symmetric` with `iter().enumerate()` to eliminate `needless_range_loop`
  5. `laplacian.rs`: fix guard message at line 68 to "graph node count {node_count} exceeds internal guard of 256; graph is designed for 15-30 nodes"
  6. `signals.rs`: delete duplicate `fn now_ns()` (line 283); replace calls with `crate::graph::now_ns()`
  7. `service.rs`: change `connected_components: 1` to `connected_components: 0` + `// TODO: compute connected components from graph`
- **Verification**: `cd sheaf-engine && cargo clippy --all-targets 2>&1 | grep -c "needless_range_loop"` produces `0`
- **Verification**: `cd sheaf-engine && cargo test` produces `test result: ok. 12 passed`
- **Commit message**: `fix: canonial now_ns, clippy-clean is_symmetric, fix guard message, honest connected_components`

### CP-2: Wire config threshold into signal extraction
- **Touches**: `sheaf-engine/src/signals.rs`
- **Tasks**:
  1. `signals.rs`: in `extract_arbitrage_signals`, read threshold from `graph.config().arbitrage_signal_threshold_bps` instead of hardcoded `5.0`
- **Verification**: `cd sheaf-engine && cargo test` — write a test that sets `arbitrage_signal_threshold_bps` to a non-default value and verifies signal emission matches
- **Verification**: `cd sheaf-engine && cargo clippy --all-targets && cargo test` — zero warnings, all tests pass
- **Commit message**: `fix: wire GraphConfig.arbitrage_signal_threshold_bps into signal extraction`
