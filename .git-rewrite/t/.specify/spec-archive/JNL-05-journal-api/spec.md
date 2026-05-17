# Specification: Journal CRUD API Endpoints

**Spec ID:** JNL-05-journal-api
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / API
**Priority:** P0 — frontend needs endpoints
**Depends on:** JNL-01-schema, JNL-02-ingestion
**Series:** Batch 3 — Backend API (JNL-05, JNL-06)

---

## Problem Statement

The journal data exists in PostgreSQL but has no HTTP API for the frontend to consume. We need CRUD endpoints for trades (read + annotate), journal entries (full CRUD), and tags (full CRUD). All endpoints are user-scoped via JWT auth.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | List journal trades with pagination, sorting, filtering | High | routes/journal.rs |
| FR-2 | Get single trade with linked entries and tags | High | routes/journal.rs |
| FR-3 | Update trade notes (inline quick note) | Medium | routes/journal.rs |
| FR-4 | Full CRUD for journal entries (markdown notes) | High | routes/journal.rs |
| FR-5 | Full CRUD for tags | High | routes/journal.rs |
| FR-6 | Add/remove tags from trades | High | routes/journal.rs |
| FR-7 | All endpoints authenticated via existing JWT middleware | High | routes/journal.rs |

---

## Technical Implementation

### Endpoints

```
# Trades (read + annotate only — writes come from ingestion pipeline)
GET    /api/v1/journal/trades              — list trades (paginated, filtered)
GET    /api/v1/journal/trades/:id          — single trade with entries + tags
PATCH  /api/v1/journal/trades/:id/notes    — update inline note

# Journal Entries (full CRUD)
GET    /api/v1/journal/entries             — list entries (paginated, filtered)
POST   /api/v1/journal/entries             — create entry
GET    /api/v1/journal/entries/:id         — get entry
PUT    /api/v1/journal/entries/:id         — update entry
DELETE /api/v1/journal/entries/:id         — delete entry

# Tags
GET    /api/v1/journal/tags                — list user's tags
POST   /api/v1/journal/tags               — create tag
PUT    /api/v1/journal/tags/:id            — update tag (rename, recolor)
DELETE /api/v1/journal/tags/:id            — delete tag

# Trade-Tag linking
POST   /api/v1/journal/trades/:id/tags     — add tags to trade
DELETE /api/v1/journal/trades/:id/tags/:tag_id  — remove tag from trade
```

### Request/Response Shapes

**List trades:**
```
GET /api/v1/journal/trades?page=1&limit=50&exchange=woo&symbol=BTC_USDT&side=LONG&date_from=2026-01-01&date_to=2026-03-17&tag=revenge-trade&sort=closed_at&order=desc
```

```json
{
  "trades": [...],
  "total": 234,
  "page": 1,
  "limit": 50
}
```

**Create journal entry:**
```json
POST /api/v1/journal/entries
{
  "trade_id": "uuid-or-null",
  "entry_date": "2026-03-17",
  "title": "Post-trade review: BTC short",
  "body": "## What went right\n- Entry was clean...",
  "entry_type": "post-trade"
}
```

**Create tag:**
```json
POST /api/v1/journal/tags
{
  "name": "revenge-trade",
  "color": "#FF003C"
}
```

**Add tags to trade:**
```json
POST /api/v1/journal/trades/:id/tags
{
  "tag_ids": ["uuid1", "uuid2"]
}
```

### Route Registration

In `main.rs`, add under the existing authenticated scope:

```rust
web::scope("/api/v1/journal")
    .route("/trades", web::get().to(journal::list_trades))
    .route("/trades/{id}", web::get().to(journal::get_trade))
    .route("/trades/{id}/notes", web::patch().to(journal::update_trade_notes))
    .route("/trades/{id}/tags", web::post().to(journal::add_trade_tags))
    .route("/trades/{id}/tags/{tag_id}", web::delete().to(journal::remove_trade_tag))
    .route("/entries", web::get().to(journal::list_entries))
    .route("/entries", web::post().to(journal::create_entry))
    .route("/entries/{id}", web::get().to(journal::get_entry))
    .route("/entries/{id}", web::put().to(journal::update_entry))
    .route("/entries/{id}", web::delete().to(journal::delete_entry))
    .route("/tags", web::get().to(journal::list_tags))
    .route("/tags", web::post().to(journal::create_tag))
    .route("/tags/{id}", web::put().to(journal::update_tag))
    .route("/tags/{id}", web::delete().to(journal::delete_tag))
```

### Files

- `testudo-exchange/crates/router/src/routes/journal.rs` — new (all handlers)
- `testudo-exchange/crates/router/src/main.rs` — register routes

---

## Acceptance Criteria

- [ ] All 15 endpoints respond correctly
- [ ] List trades supports pagination (page + limit params)
- [ ] List trades supports filtering by: exchange, symbol, side, date range, tags
- [ ] List trades supports sorting by: closed_at, net_pnl, r_multiple, duration_secs
- [ ] All endpoints enforce user-scoping (user can only see own data)
- [ ] Create/update entry validates: title non-empty, body non-empty, entry_type is valid enum
- [ ] Delete tag cascades: removes from all trade-tag links
- [ ] 404 for non-existent resources, 403 for other user's resources
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Completion Signal

This spec is complete when:
1. All CRUD endpoints work via curl/httpie
2. All acceptance criteria met
3. Code committed to master
