# Specification: Dashboard Layout + Overview Panel

**Spec ID:** JNL-07-dashboard-layout
**Date:** 2026-03-17
**Status:** Draft
**Class:** Feature / Frontend
**Priority:** P1 — visual shell for all journal features
**Depends on:** JNL-06-analytics-api
**Series:** Batch 4 — Frontend Dashboard (JNL-07, JNL-08)

---

## Problem Statement

The journal has a backend API but no frontend. We need the app shell, routing, and the primary overview panel — the first thing a trader sees when they open the journal. This panel displays account stats, performance metrics, and risk stats in the brutalist dark aesthetic.

---

## User Stories

- **As a trader**, I want to see my key stats at a glance when I open the journal.
- **As a trader**, I want to filter all views by exchange, symbol, and date range.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Solid.js app shell with dark theme (brutalist Testudo aesthetic) | High | testudo-journal/ |
| FR-2 | Tab/route navigation: Overview, Charts, Trades, Journal | High | routing |
| FR-3 | Global filter bar: exchange selector, symbol search, date range | High | layout |
| FR-4 | Account overview card: total P&L, net P&L, fees, trade count | High | overview |
| FR-5 | Performance stats card: win rate, profit factor, expectancy, avg R, trades/day | High | overview |
| FR-6 | Risk stats card: max drawdown, worst day/week/month, streaks | High | overview |
| FR-7 | Fetch data from `/api/v1/journal/analytics/overview` | High | data layer |

---

## Technical Implementation

### App Structure

```
testudo-journal/
├── src/
│   ├── index.tsx               — Solid.js entry
│   ├── App.tsx                 — shell + routing
│   ├── api/
│   │   └── client.ts           — fetch wrapper with JWT auth
│   ├── components/
│   │   ├── Layout.tsx          — sidebar/header + main content
│   │   ├── FilterBar.tsx       — global filters (exchange, symbol, dates)
│   │   ├── StatCard.tsx        — reusable stat display card
│   │   └── Overview.tsx        — overview page assembling stat cards
│   ├── lib/
│   │   └── formatters.ts       — number/currency/percentage formatters
│   └── styles/
│       └── app.css             — Tailwind + theme tokens
├── index.html
├── package.json
├── tsconfig.json
├── tailwind.config.ts
└── vite.config.ts
```

### Design System (matches web app + extension)

```css
/* Core tokens */
--color-bg-core: #050505;
--color-bg-panel: #0A0A0A;
--color-bg-elevated: #111111;
--color-border: #3F3F46;
--color-signal-green: #00FF41;
--color-signal-red: #FF003C;
--color-text-primary: #ffffff;
--color-text-secondary: #888888;
--color-text-dim: #555555;

/* Typography */
font-sans: "Space Grotesk", system-ui, sans-serif;
font-mono: "Space Mono", ui-monospace, monospace;
```

### Overview Layout

Three stat cards in a row, each with a header label and grid of key-value pairs:

```
┌─────────────────────┬─────────────────────┬─────────────────────┐
│  ACCOUNT            │  PERFORMANCE        │  RISK               │
│                     │                     │                     │
│  Total P&L  $1,434  │  Win Rate    58.5%  │  Max DD      8.7%   │
│  Net P&L    $1,345  │  Profit F     1.82  │  Worst Day   -$156  │
│  Fees         $89   │  Expectancy  $15.1  │  Worst Week  -$234  │
│  Trades       234   │  Avg R        1.45  │  Streak       +3    │
│                     │  Trades/Day   2.3   │  Best Streak  +8    │
└─────────────────────┴─────────────────────┴─────────────────────┘
```

- P&L values: green (#00FF41) for positive, red (#FF003C) for negative
- Numbers in `Space Mono`, labels in `Space Grotesk`
- Card borders: `1px solid #3F3F46`, border-radius: `8px`
- Background: `#0A0A0A`, elevated cards on `#111111`

### API Client

```typescript
// src/api/client.ts
const API_BASE = import.meta.env.VITE_API_URL || "http://127.0.0.1:8080";

export async function fetchOverview(filters: StatsFilter): Promise<OverviewResponse> {
  const params = new URLSearchParams();
  if (filters.exchange) params.set("exchange", filters.exchange);
  if (filters.symbol) params.set("symbol", filters.symbol);
  if (filters.dateFrom) params.set("date_from", filters.dateFrom);
  if (filters.dateTo) params.set("date_to", filters.dateTo);

  const res = await fetch(`${API_BASE}/api/v1/journal/analytics/overview?${params}`, {
    headers: { Authorization: `Bearer ${getToken()}` },
  });
  return res.json();
}
```

### Files

- `testudo-journal/` — entire new Solid.js application
- All files listed in App Structure above

---

## Acceptance Criteria

- [ ] Solid.js app builds with `bun run build`
- [ ] Dark theme matches Testudo web app aesthetic
- [ ] Overview page displays all stats from API
- [ ] Filter bar sends params to API and refreshes data
- [ ] Space Grotesk for labels, Space Mono for numbers
- [ ] Positive P&L green, negative red
- [ ] Tab navigation between Overview/Charts/Trades/Journal routes
- [ ] Responsive: works at 1024px+ width

---

## Completion Signal

This spec is complete when:
1. Journal app loads and displays overview stats
2. Visual style matches Testudo brutalist aesthetic
3. All acceptance criteria met
4. Code committed to master
