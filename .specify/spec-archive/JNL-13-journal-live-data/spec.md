# JNL-13: Wire Journal Service for Live Trade Capture

## Problem
The entire journal data pipeline exists but is silently disabled. `JournalService` is never passed to `FillDetectorService` during initialization in `main.rs`. When trades close, `fire_journal_write()` checks `self.journal_service.is_some()` and skips the write.

## Root Cause
`main.rs` line ~680: `FillDetectorService::new()` is called without `.with_journal_service()`.

## Changes

### FR-1: Wire JournalService to FillDetector (main.rs)
- Instantiate `JournalService::new(pg_pool.clone())`
- Call `.with_journal_service(journal_service)` on the FillDetectorService builder

## Data Flow (once wired)
```
Trade closes on exchange
  → FillDetector.handle_order_update()
  → fire_journal_write() [tokio::spawn]
  → JournalService.record_trade_close()
  → INSERT journal_trades + UPSERT journal_daily_stats
  → GET /journal/analytics/* serves live data to frontend
```

## Verification
- `cargo clippy --all-targets && cargo test`
