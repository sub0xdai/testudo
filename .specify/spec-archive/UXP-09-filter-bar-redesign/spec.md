# Specification: FilterBar Redesign — Data-Driven Dropdowns & Time Presets

**Spec ID:** UXP-09-filter-bar-redesign
**Date:** 2026-03-18
**Status:** Draft
**Class:** Feature / UX
**Priority:** P1 — Core navigation control used on every page
**Depends on:** UXP-08-accessibility-baseline
**Series:** UXP-01 through UXP-09 (Journal UX Polish from design critique)

---

## Problem Statement

The FilterBar uses a hardcoded exchange dropdown (WOO, BINANCE, HYPERLIQUID), a free-text symbol input prone to typos, and raw `<input type="date">` fields that render inconsistently across browsers. Users must type exact symbol names with no discoverability of what pairs exist in their data. The APPLY/CLEAR button pattern adds unnecessary friction — every filter change requires an extra click.

Trading dashboards like FXBlue, Myfxbook, and TradingView use data-driven dropdowns populated from actual trade history, time preset buttons for common ranges, and immediate-apply behavior. This spec brings the FilterBar to that standard.

---

## User Stories

- **As a trader reviewing performance**, I want to select from symbols I've actually traded, so I don't have to remember exact pair names or worry about typos.
- **As a trader filtering by exchange**, I want the symbol list to cascade — only showing pairs traded on the selected exchange.
- **As a trader checking recent performance**, I want one-click time presets (1W, 1M, 3M, YTD, ALL) instead of manually entering date ranges.

---

## Design

### Layout

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ EXCHANGE [ALL ▾]   SYMBOL [ALL ▾]   │ 1W  1M  3M  YTD [ALL]  CUSTOM │ × reset │
└──────────────────────────────────────────────────────────────────────────────┘
```

When CUSTOM is active, date inputs appear inline:

```
│ 1W  1M  3M  YTD  ALL  [CUSTOM]  FROM [Mar 1 📅]  TO [Mar 18 📅] │ × reset │
```

### Interaction Model

- **Immediate apply** — every filter change updates `FilterContext` instantly (no APPLY button)
- **Cascaded dropdowns** — selecting an exchange re-fetches filter-options to populate symbols for that exchange
- **Reset** — single `× reset` link clears all filters to defaults (exchange=ALL, symbol=ALL, date=ALL)
- **Debounce** — symbol search input debounced 300ms to avoid excessive client-side filtering

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | New `GET /api/v1/journal/analytics/filter-options?exchange=X` endpoint returning `{ exchanges: string[], symbols: { symbol: string, count: number }[] }` | High | Backend |
| FR-2 | Exchanges list derived from `SELECT DISTINCT exchange FROM trades WHERE user_id = $1` — always returns all exchanges regardless of exchange param | High | Backend |
| FR-3 | Symbols list derived from `SELECT symbol, COUNT(*) FROM trades WHERE user_id = $1 AND (exchange = $2 OR $2 IS NULL) GROUP BY symbol ORDER BY count DESC` | High | Backend |
| FR-4 | Exchange dropdown populated from filter-options endpoint (data-driven, not hardcoded) with "ALL" as default first option | High | Frontend |
| FR-5 | New `SymbolSearch` component: searchable dropdown with case-insensitive substring matching, showing `SYMBOL (count)` format, "ALL" as default | High | Frontend |
| FR-6 | Selecting an exchange immediately re-fetches filter-options to cascade the symbol list; if the currently selected symbol doesn't exist on the new exchange, reset symbol to ALL | High | Frontend |
| FR-7 | Time preset buttons: 1W (-7d), 1M (-30d), 3M (-90d), YTD (Jan 1), ALL (no date filter). Active preset highlighted with `text-signal-green` and bottom border | High | Frontend |
| FR-8 | CUSTOM preset reveals two native `<input type="date">` inline for arbitrary range selection. Selecting any other preset hides the date inputs | Medium | Frontend |
| FR-9 | All filter changes apply immediately to `FilterContext` — remove APPLY and CLEAR buttons | High | Frontend |
| FR-10 | Single `× reset` link that clears all filters to default state (exchange=ALL, symbol=ALL, preset=ALL) | High | Frontend |
| FR-11 | SymbolSearch keyboard navigation: Arrow Up/Down moves highlight, Enter selects, Escape closes dropdown | Medium | Frontend |
| FR-12 | SymbolSearch `aria-haspopup="listbox"`, `aria-expanded`, `role="listbox"` on dropdown, `role="option"` on items (consistent with UXP-08 patterns) | Medium | Frontend |

---

## Technical Implementation

### Backend: filter-options Endpoint

Add to existing `crates/router/src/routes/journal_analytics.rs`:

```rust
#[derive(Deserialize)]
pub struct FilterOptionsQuery {
    pub exchange: Option<String>,
}

#[derive(Serialize)]
pub struct FilterOptionsResponse {
    pub exchanges: Vec<String>,
    pub symbols: Vec<SymbolCount>,
}

#[derive(Serialize)]
pub struct SymbolCount {
    pub symbol: String,
    pub count: i64,
}

async fn filter_options(
    user: AuthUser,
    query: web::Query<FilterOptionsQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let exchanges = sqlx::query_scalar!(
        "SELECT DISTINCT exchange FROM trades WHERE user_id = $1 ORDER BY exchange",
        user.id
    ).fetch_all(pool.as_ref()).await?;

    let symbols = sqlx::query_as!(
        SymbolCount,
        r#"SELECT symbol, COUNT(*) as "count!" FROM trades
           WHERE user_id = $1 AND ($2::text IS NULL OR exchange = $2)
           GROUP BY symbol ORDER BY count DESC"#,
        user.id, query.exchange
    ).fetch_all(pool.as_ref()).await?;

    Ok(HttpResponse::Ok().json(FilterOptionsResponse { exchanges, symbols }))
}
```

Register route alongside existing journal analytics routes.

### Frontend: SymbolSearch Component

```tsx
// testudo-journal/src/components/SymbolSearch.tsx
export function SymbolSearch(props: {
  symbols: { symbol: string; count: number }[]
  value: string          // current selected symbol or ''
  onSelect: (symbol: string) => void
})
```

- On click/focus: open dropdown, show all symbols
- On type: filter list with case-insensitive substring match
- "ALL" is always first item (count = total of all symbols)
- Click or Enter selects, Escape closes
- Full ARIA attributes per UXP-08 pattern

### Frontend: FilterBar Rewrite

```tsx
// Simplified structure
function FilterBar() {
  const { filters, setFilters } = useFilters()
  const [options] = createResource(() => filters().exchange, fetchFilterOptions)
  const [preset, setPreset] = createSignal<string>('all')
  const [showCustom, setShowCustom] = createSignal(false)

  function selectPreset(key: string) {
    setPreset(key)
    if (key === 'custom') {
      setShowCustom(true)
      return
    }
    setShowCustom(false)
    const dateFrom = computeDateFrom(key) // 1w=-7d, 1m=-30d, etc.
    setFilters({ ...filters(), dateFrom, dateTo: undefined })
  }

  function selectExchange(exchange: string) {
    setFilters({ ...filters(), exchange: exchange || undefined, symbol: undefined })
  }

  function selectSymbol(symbol: string) {
    setFilters({ ...filters(), symbol: symbol || undefined })
  }

  function reset() {
    setFilters({})
    setPreset('all')
    setShowCustom(false)
  }
}
```

### FilterContext Change

Add optional `datePreset` to `StatsFilter` for UI state tracking (not sent to API):

```ts
export interface StatsFilter {
  exchange?: string
  symbol?: string
  dateFrom?: string
  dateTo?: string
}
```

No change to the interface — preset state is local to FilterBar component.

### Files

| File | Change |
|------|--------|
| `testudo-exchange/crates/router/src/routes/journal_analytics.rs` | New `filter_options` handler + route registration |
| `testudo-exchange/crates/router/src/repositories/journal.rs` | New `get_filter_options` query (or inline in handler) |
| `testudo-journal/src/api/client.ts` | New `fetchFilterOptions(exchange?)` function + `FilterOptions` type |
| `testudo-journal/src/components/FilterBar.tsx` | Complete rewrite — data-driven dropdowns, presets, immediate apply |
| `testudo-journal/src/components/SymbolSearch.tsx` | New searchable dropdown component |
| `testudo-journal/src/components/filterContext.tsx` | No interface change (preset is local state) |

---

## Acceptance Criteria

- [ ] `GET /api/v1/journal/analytics/filter-options` returns distinct exchanges and symbol counts from trade history
- [ ] `?exchange=X` param filters symbols to only those traded on that exchange
- [ ] Exchange dropdown is data-driven (populated from API, not hardcoded)
- [ ] Symbol dropdown shows searchable list with `SYMBOL (count)` format
- [ ] Substring search filters symbols case-insensitively
- [ ] Selecting exchange cascades symbol list; resets symbol to ALL if current symbol not available on new exchange
- [ ] Time presets (1W, 1M, 3M, YTD, ALL) apply date filters immediately on click
- [ ] CUSTOM reveals inline date inputs; selecting another preset hides them
- [ ] No APPLY/CLEAR buttons — all changes are immediate
- [ ] `× reset` clears all filters to defaults
- [ ] SymbolSearch has keyboard navigation (arrows, Enter, Escape)
- [ ] SymbolSearch has ARIA attributes (listbox, option, expanded)
- [ ] `cargo clippy --all-targets && cargo test` passes
- [ ] `bun run build` passes (testudo-journal)

---

## Risks

1. **Filter-options query performance with large trade history** — Mitigated: indexed on `(user_id, exchange)`, query runs 1-3 times per session.
2. **Symbol names differ between exchanges** — By design: cascading ensures you only see symbols for the selected exchange. Under "ALL", duplicates with same name are merged (combined count).
3. **Custom date inputs render differently per browser** — Acceptable trade-off: presets handle 90% of use cases, native date pickers are functional and accessible.

---

## Completion Signal

This spec is complete when:
1. Backend endpoint returns data-driven filter options
2. FilterBar uses cascaded data-driven dropdowns with searchable symbols
3. Time presets work with immediate apply
4. No APPLY/CLEAR buttons remain
5. All builds pass
6. Code committed to master
