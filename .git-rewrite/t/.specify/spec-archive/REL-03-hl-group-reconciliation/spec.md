# Specification: HL OrderGroup Reconciliation on Position Close

**Spec ID:** REL-03-hl-group-reconciliation
**Date:** 2026-03-31
**Status:** Draft
**Class:** Core / State Management
**Priority:** P1 — Active positions show stale state; orphaned trigger orders remain on exchange
**Depends on:** REL-02-hl-journal-pipeline
**Series:** REL-02 through CON-01a (HL data pipeline fix + CON-01 regression cleanup)

---

## Problem Statement

When REL-02 writes an HL closing fill to the journal, the corresponding OrderGroup in the shadow engine remains in `Active` status because FillDetector never matched the fill. This causes three visible problems:

1. **Stale active positions** — The extension popup and position cards show positions that are already flat on-exchange. The trader sees phantom positions that don't exist.
2. **Orphaned trigger orders** — When an SL fills, the corresponding TP trigger stays live on HL (and vice versa). These are `reduce_only` so they can't open new positions, but they clutter the order book and confuse the trader.
3. **No extension notifications** — The "stopped out" / "took profit" toast never fires because `broadcast_fill_event()` only runs inside FillDetector's matched-fill path.

This spec adds a background reconciliation step that runs after REL-02 writes a journal entry. It finds the matching OrderGroup by `(user_id, symbol)`, transitions it to terminal state, best-effort cancels sibling orders, and emits the UI notification.

This is deliberately a **separate spec from REL-02** because the journal write is the critical path (P0) while group cleanup is a quality-of-life improvement (P1). REL-02 works correctly even if REL-03 is not implemented — the journal and charts are accurate, just the live position state lags.

---

## User Stories

- **As a trader**, I want closed positions to disappear from my active positions list, so that I only see positions that are actually open on-exchange.
- **As a trader**, I want orphaned SL/TP orders cancelled when the other side fills, so that my order book stays clean.
- **As a trader**, I want a toast notification when my HL trade closes, so that I know immediately without checking the journal.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | After writing a journal entry (REL-02), query active OrderGroups for `(user_id, symbol)`. If exactly 1 match, transition to terminal state (`StoppedOut` or `TookProfit` based on fill side). | High | ws_fills.rs / engine |
| FR-2 | Best-effort cancel sibling orders on HL for the matched group (entry, SL, TP exchange_order_ids). `OrderNotFound` is a no-op. | Medium | ws_fills.rs / exchange_api |
| FR-3 | Emit `broadcast_fill_event()` equivalent pg_notify after group transition for extension UI update. | Medium | ws_fills.rs |
| FR-4 | If 0 or 2+ active groups match `(user_id, symbol)`, skip group cleanup entirely. Journal entry is still written (REL-02). Log at warn level for observability. | High | ws_fills.rs |
| FR-5 | If group cleanup fails (engine error, cancel timeout), the journal entry is NOT rolled back. Cleanup is best-effort fire-and-forget. | High | ws_fills.rs |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Query active groups by (user_id, symbol) via EngineHandle, transition to terminal if unambiguous | Group status changes from Active to terminal |
| CP-2 | Cancel sibling orders on HL via ExchangeApi | Orphaned orders removed from exchange |
| CP-3 | Emit pg_notify for extension UI | Toast fires in extension on trade close |

### Matching Strategy

The reconciler does NOT do fuzzy matching on size/side. It uses a strict **cardinality check**:

```rust
let active_groups = engine_handle.get_active_groups(user_id).await;
let matching: Vec<_> = active_groups.iter()
    .filter(|g| g.symbol == symbol)
    .collect();

match matching.len() {
    1 => {
        // Unambiguous — safe to transition
        let group = &matching[0];
        let terminal_status = if fill_side == "LONG" {
            // Closing a long → was it SL or TP?
            // If exit_price < entry_price → stopped out, else took profit
            if exit_price < group.entry_price.unwrap_or(exit_price) {
                OrderGroupStatus::StoppedOut
            } else {
                OrderGroupStatus::TookProfit
            }
        } else {
            if exit_price > group.entry_price.unwrap_or(exit_price) {
                OrderGroupStatus::StoppedOut
            } else {
                OrderGroupStatus::TookProfit
            }
        };
        engine_handle.update_group_status(group.id, terminal_status).await;
        // Cancel siblings, emit notify...
    }
    0 => {
        // No matching group — trade was placed outside testudo or group already cleaned up.
        // This is normal for import-only trades. No action needed.
    }
    _ => {
        // Ambiguous — multiple active groups for same symbol.
        // Log and skip. Don't risk closing the wrong one.
        tracing::warn!(
            user_id = %user_id,
            symbol = %symbol,
            count = matching.len(),
            "REL-03: ambiguous group match, skipping cleanup"
        );
    }
}
```

### Why Cardinality Check is Safe

Most traders have 0-1 active positions per symbol on HL at any time. HL doesn't support hedging (can't be long and short same symbol simultaneously). So `matching.len() == 1` is the common case. The `2+` case only happens if testudo has stale ghost groups from prior crashes — the skip is the correct safety valve.

### Dependency Injection

`HyperliquidFillSubscriber` needs access to `EngineHandle` and `ExchangeApi` for group cleanup. These are passed through `WsSubscriptionManager` → subscriber constructor:

```rust
pub fn new(
    network: Network,
    user_address: Address,
    order_update_sender: mpsc::Sender<OrderUpdateEvent>,
    // REL-02 additions:
    journal: Option<JournalService>,
    user_id: Option<Uuid>,
    notify_pool: Option<PgPool>,
    // REL-03 additions:
    engine_handle: Option<EngineHandle>,
    exchange_api: Option<Arc<dyn ExchangeApi>>,
) -> Self
```

### Paved Roads

- `fill_detector.rs:321-329` — Existing `cancel_all_related_orders()` pattern (reuse logic)
- `fill_detector.rs:330-335` — Existing `broadcast_fill_event()` pattern (reuse pg_notify format)
- `engine_handle.get_active_groups(user_id)` — Already exists
- `engine_handle.update_group_status(group_id, status)` — Already exists

### Files

- `crates/router/src/services/hyperliquid/ws_fills.rs` — Add group reconciliation after journal write
- `crates/router/src/services/ws_subscription_manager.rs` — Pass EngineHandle + ExchangeApi to subscriber
- `crates/router/src/main.rs` — Wire new dependencies through to WsSubscriptionManager

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] After HL SL fill: matching OrderGroup transitions to `StoppedOut`, sibling TP cancelled
- [ ] After HL TP fill: matching OrderGroup transitions to `TookProfit`, sibling SL cancelled
- [ ] Extension receives pg_notify and shows toast notification
- [ ] Active positions list no longer shows closed HL positions
- [ ] Ambiguous matches (2+ groups same symbol) are logged and skipped safely
- [ ] No-match case (0 groups) is silent (normal for import-only trades)
- [ ] Journal entry is never rolled back on cleanup failure
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Wrong group closed** — If 2 groups match the same symbol, we skip rather than guess. Mitigation: cardinality check. Wrong closure is worse than stale display.

2. **Cancel fails silently** — HL cancel call may timeout or return error. Mitigation: best-effort with warn log. The trigger order is reduce_only so it can't open a new position — just clutter.

3. **Race with FillDetector** — If the entry fill's OID DID match (entries work), FillDetector already processed it. The reconciler checks terminal status first — if the group is already terminal, it's a no-op.

4. **EngineHandle in subscriber** — Increases the subscriber's dependency surface. Mitigation: `Option<EngineHandle>` — when None, skip cleanup entirely. REL-02 still works.

---

## Completion Signal

This spec is complete when:
1. Closed HL positions disappear from active positions within 30s
2. Orphaned sibling orders are cancelled on HL
3. Extension toast notifications fire for HL trade closes
4. All acceptance criteria pass
5. Code committed to master
