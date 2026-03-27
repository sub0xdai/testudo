# Specification: Live Thesis Journal — Write Before Close

**Spec ID:** JNL-20-live-thesis-journal
**Date:** 2026-03-27
**Status:** Draft
**Class:** Feature / Frontend + Backend
**Priority:** P0 — Traders need to document their thesis while in a trade, not after. The current journal only shows closed trades, making it impossible to write pre-trade analysis or track the reasoning behind active positions.
**Depends on:** JNL-19-journal-consolidation
**Series:** JNL-20 (standalone)

---

## Problem Statement

The journal table only shows rows from `journal_trades` — which are created by `FillDetectorService` when a position **closes**. A trader who enters a position via the extension has no way to:

1. See the active trade on the Desk
2. Write a thesis for why they entered
3. Attach tags or notes before the trade closes
4. Review their pre-trade reasoning against the outcome

The extension popup shows active positions, but the Desk (where journaling happens) does not. The journal should be the single place to document a trade's entire lifecycle: thesis → entry → management → exit → review.

---

## User Stories

- **As a trader**, I want to see my active positions on the Desk journal page, so that I can write about them while I'm in the trade.
- **As a trader**, I want to write a pre-trade thesis and attach it to my active position, so that I can review my reasoning after the trade closes.
- **As a trader**, I want my thesis to carry over when the trade closes, so that I don't lose my notes.
- **As a trader**, I want to export my thesis + trade result as a single `.md` file, so that I have a complete record.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Add an "ACTIVE" section above the closed trades table on the Journal page. This section shows open trade groups from `GET /trades` (same endpoint the extension uses). | High | testudo-journal |
| FR-2 | Each active trade row shows: symbol, side, entry price, current status (pending/active), time in trade. Clicking opens the same detail sidebar. | High | testudo-journal |
| FR-3 | The detail sidebar works for active trades — notes and tags can be saved against the trade group ID before the trade closes. | High | testudo-journal + Backend |
| FR-4 | Backend: Allow `PATCH /journal/trades/{id}/notes` and `POST /journal/trades/{id}/tags` to accept a `trade_group_id` that doesn't yet exist in `journal_trades`. Store notes/tags linked to the group ID, and when the trade closes and `record_trade_close()` runs, merge the pre-existing notes/tags. | High | Backend |
| FR-5 | When a trade closes, `record_trade_close()` checks for pre-existing notes/tags on the `trade_group_id` and carries them over to the new `journal_trades` row. | High | Backend |
| FR-6 | Active trades are visually distinct from closed trades — different section header, pulsing status indicator, no P&L column (not yet known). | Medium | testudo-journal |
| FR-7 | Auto-refresh: active positions poll every 30s or update via WebSocket `order.*` events. | Medium | testudo-journal |

---

## Technical Implementation

### FR-1 + FR-2: Active Positions Section

The Journal page (`Trades.tsx`) already renders `TradeTable`. Add an `ActivePositions` component above it that calls the same `GET /trades` endpoint the extension uses:

```typescript
// Fetch active trade groups
const [activeGroups] = createResource(async () => {
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/trades`)
  if (!res.ok) return []
  const data = await res.json()
  // Normalize: backend returns { success, data: TradeGroupResponse[] }
  return data.data || []
})
```

Each active trade renders as a compact card or row:
```
● BTC_USDT  SHORT  Entry: 69,096  ACTIVE  2h ago
```

Clicking opens the existing `TradeDetail` sidebar — but keyed on `trade_group_id` instead of `journal_trades.id`.

### FR-3 + FR-4: Pre-Close Notes/Tags

The sidebar needs to work with either:
- A `journal_trades.id` (closed trade — existing behavior)
- A `trade_group_id` (active trade — new behavior)

**Backend approach:** Create a `journal_trade_drafts` table (or reuse `journal_entries` with a `trade_group_id` link):

```sql
CREATE TABLE journal_trade_drafts (
    trade_group_id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id),
    notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Tags can use the existing `journal_trade_tags` table — but reference the `trade_group_id` before the `journal_trades` row exists. Since `journal_trade_tags.trade_id` references `journal_trades(id)`, we need a lightweight workaround:

**Simpler approach:** Store draft notes/tags in `journal_entries` as a `pre-trade` type entry linked by trade_group_id (stored in a new column or in the entry body as metadata). When the trade closes, `record_trade_close()` looks up entries with matching `trade_group_id` and links them.

**Simplest approach:** Add `notes` and a pre-create hook to the trade group itself:
- `PATCH /api/v1/trades/{group_id}/notes` — already exists for closed trades, extend to accept group IDs for active trades by writing to `journal_trade_drafts`
- When `record_trade_close()` fires, check `journal_trade_drafts` for matching `trade_group_id`, copy `notes` to the new `journal_trades` row, delete the draft

### FR-5: Merge on Close

In `journal_service.rs::record_trade_close()`:

```rust
// After inserting the journal_trades row:
if let Some(group_id) = event.trade_group_id {
    // Check for pre-existing draft notes
    if let Ok(Some(draft)) = sqlx::query_as::<_, DraftRow>(
        "DELETE FROM journal_trade_drafts WHERE trade_group_id = $1 RETURNING notes"
    ).bind(group_id).fetch_optional(&self.pool).await {
        if let Some(notes) = draft.notes {
            sqlx::query("UPDATE journal_trades SET notes = $1 WHERE id = $2")
                .bind(notes).bind(trade.id).execute(&self.pool).await.ok();
        }
    }
}
```

### FR-6: Visual Distinction

Active section uses a pulsing green dot, "ACTIVE" header, and shows entry info without P&L:

```
┌────────────────────────────────────────────────┐
│ ● ACTIVE  1                                     │
│                                                  │
│ BTC_USDT   HYP   SHORT   Entry: 69,096   2h    │
└────────────────────────────────────────────────┘
```

### Files

**New:**
- `testudo-journal/src/components/trades/ActivePositions.tsx` — Active trades section
- `crates/sqlx_postgres/migrations/YYYYMMDD_journal_trade_drafts.up.sql` — Draft notes table
- `crates/router/src/routes/trade_drafts.rs` — PATCH/GET draft notes for active trades

**Modified:**
- `testudo-journal/src/pages/Trades.tsx` — Mount ActivePositions above TradeTable
- `testudo-journal/src/components/trades/TradeDetail.tsx` — Accept trade_group_id for active trades
- `crates/router/src/services/journal_service.rs` — Merge drafts on close
- `testudo-journal/src/api/client.ts` — Add fetchActiveTrades(), saveDraftNotes()

---

## Acceptance Criteria

- [ ] Active positions appear on the Journal page above closed trades
- [ ] Clicking an active position opens the detail sidebar
- [ ] Notes can be saved against an active trade (before close)
- [ ] Tags can be attached to an active trade (before close)
- [ ] When the trade closes, pre-written notes carry over to the journal_trades row
- [ ] Export .md works for active trades (includes thesis, no P&L yet)
- [ ] Active section auto-refreshes
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] `bun run build` passes for testudo-journal

---

## Risks

1. **Trade group ID mismatch** — The extension creates trade groups with UUIDs. The `trade_group_id` must match between the active trade and the eventual `journal_trades` row. Mitigation: `record_trade_close()` already uses `trade_group_id` for idempotency — same ID flows through.
2. **Orphaned drafts** — If a trade is cancelled (never closes), the draft stays in `journal_trade_drafts`. Mitigation: Periodic cleanup of drafts older than 7 days with no matching closed trade. Or keep them — they're useful as cancelled-trade notes.

---

## Completion Signal

This spec is complete when:
1. A trader can place a trade via the extension, see it on the Desk journal, write a thesis, and have it persist through close
2. All acceptance criteria met
3. Verification commands pass
4. Code committed to master
