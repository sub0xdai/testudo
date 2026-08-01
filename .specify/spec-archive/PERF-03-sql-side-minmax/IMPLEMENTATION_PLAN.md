# PERF-03-sql-side-minmax — Implementation Plan

## Current State Summary

The implementation was completed before the plan was formalized.
`fetch_rolling_extremes` in `journal_stats.rs` already uses SQL-side
MIN/MAX via a subquery wrapper with `fetch_one`, returning a 2-tuple
`(Decimal, Decimal)` instead of the previous `fetch_all` + `Vec<RollingRow>`.
The `RollingRow` struct has been removed. All 740 tests pass; the 2
test failures are pre-existing and unrelated to this change.

The only remaining task is updating `DATABASE_AUDIT.md` to mark
Action 5 as complete.

## Checkpoints

### CP-1: Mark Action 5 complete in DATABASE_AUDIT.md ✅
- **Touches**: `DATABASE_AUDIT.md`
- **Tasks**:
  1. Mark Action 5 header as `[x]` or ✅ complete
- **Verification**: `grep -c '\[x\]' DATABASE_AUDIT.md` shows Action 5 checked
- **Commit message**: `docs: mark PERF-03 Action 5 complete (sql-side minmax)`
- Completed 2026-05-11 by /skill:vox build
