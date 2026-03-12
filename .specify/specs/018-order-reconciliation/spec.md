# 018: Order Reconciliation Service


**Status:** Complete
## Problem

Testudo implements software OCO — when SL fills, the fill detector cancels the TP (and vice versa). This relies on a fragile WebSocket chain:

```
Exchange → CCXT sidecar WS → WsSubscriptionManager → FillDetector
```

If any link breaks (WS disconnect, task abort on symbol fan-in, broadcast overflow), fills are lost and sibling orders orphan on the exchange.

**Production incident**: SOL_USDT long stopped out on WooX, but TP sell at 88.37 left open — a dangerous orphan that could open an unintended short.

**Freqtrade's proven pattern**: Poll exchange every 30s, compare with local state, clean up divergence. No WebSocket dependency for correctness.

## Solution

Keep WebSocket for speed, add polling reconciliation as defense-in-depth.

### Architecture

```
ReconciliationService (30s interval)
  ├── Read active OrderGroups from shadow engine
  ├── Group by (user_id, exchange_account_id)
  ├── For each account:
  │   ├── Fetch open orders from exchange (CCXT)
  │   ├── Fetch positions from exchange (CCXT)
  │   └── Compare with local OrderGroup state
  └── For divergent groups:
      ├── Cancel orphaned sibling orders
      ├── Update OrderGroup status
      ├── Persist to DB
      └── Broadcast event to extension
```

### Reconciliation Rules

| Local State | Exchange State | Action |
|---|---|---|
| Active group | No position, SL missing, TP present | Cancel TP, mark StoppedOut |
| Active group | No position, TP missing, SL present | Cancel SL, mark TookProfit |
| Active group | No position, nothing in open orders | Mark Closed |
| Pending group | Entry not in open orders | Mark Cancelled |

### Design Decisions

- **30-second interval**: Balances API rate limits with timely orphan cleanup
- **Skip first tick**: Avoids redundant work immediately after startup rehydration
- **Only live trades**: Paper trades (no exchange_account_id) are skipped
- **Batch by account**: Single API call per account, not per group
- **Two-phase lock**: Read state → release lock → execute exchange operations
- **Shutdown-aware**: Uses CancellationToken for graceful shutdown

## Functional Requirements

- **FR-1**: Poll exchange every 30s for open orders and positions per active account
- **FR-2**: Detect orphaned sibling orders when position is closed but orders remain
- **FR-3**: Cancel orphaned orders and update group status
- **FR-4**: Persist terminal state to DB to prevent rehydration resurrection
- **FR-5**: Broadcast reconciliation events to extension via management channel
- **FR-6**: Skip paper trades (no exchange_account_id)
- **FR-7**: Graceful shutdown via CancellationToken

## Files

| File | Action |
|---|---|
| `crates/router/src/services/reconciliation.rs` | NEW — ReconciliationService |
| `crates/router/src/services/mod.rs` | MODIFY — add `pub mod reconciliation` |
| `crates/router/src/main.rs` | MODIFY — spawn reconciliation task |

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
```
