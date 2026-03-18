# UXP-16 Troubleshooting: The Last Mile to Bloomberg

**Date:** 2026-03-18
**Status:** Active diagnostic
**Context:** ECharts migration complete, charts render, but zero data flows.

---

## Current State

The frontend is **100% ready**. Five analytical charts migrated from Chart.js to ECharts v6→v5, a reusable `EChart` Solid.js wrapper handles lifecycle, the `testudo-dark` theme matches design tokens, the Vite proxy eliminates CORS, and ChartContainer error/empty/retry states render correctly.

The backend is **90% ready**. Two complete service layers exist:
- `journal_stats.rs` (690 lines) — StatsEngine with account overview, performance stats, risk stats
- `journal_timeseries.rs` (575 lines) — TimeSeriesService with all 6 chart data generators
- `journal_service.rs` — TradeCloseEvent ingestion pipeline
- Database schema: `journal_trades`, `journal_daily_stats`, `journal_entries`, `journal_tags`

**The gap is 10%: six HTTP route handlers are missing.**

```
Frontend (ECharts)     Vite Proxy        Rust Backend (Actix-web)
──────────────────     ──────────        ────────────────────────
fetchOverview()    →   /api/v1/...  →    ❌ No handler (401/404)
fetchEquityCurve() →   /api/v1/...  →    ❌ No handler
fetchDailyPnl()    →   /api/v1/...  →    ❌ No handler
fetchSymbolBreak() →   /api/v1/...  →    ❌ No handler
fetchDurationProf()→   /api/v1/...  →    ❌ No handler
fetchReturnDist()  →   /api/v1/...  →    ❌ No handler
fetchTimeDist()    →   /api/v1/...  →    ❌ No handler
fetchFilterOpts()  →   /api/v1/...  →    ✅ filter_options() exists
```

The only wired analytics route is `filter-options` (line 903 of main.rs).

---

## Self-Reflection: Hard Questions

### 1. Why did we build charts before wiring the data?

Because the spec series was UI-first: UXP-01 through UXP-16 focused on design system, accessibility, layout, and charting infrastructure. The backend JNL-05 (journal API) and JNL-06 (analytics API) specs were executed earlier but **only partially** — they built the services but stopped short of the HTTP glue.

**Lesson:** A vertical slice (one chart, end-to-end, database to pixel) would have caught this immediately. We built two horizontal layers (all backend services, then all frontend charts) that never connected.

### 2. Why 401 instead of 404?

The Actix-web auth middleware runs BEFORE route matching on the `/api/v1/journal/` scope. When a request hits a non-existent route under that scope, the middleware rejects it with 401 before Actix can return 404. This masked the real problem — we thought it was auth, not missing routes.

### 3. What does "Bloomberg terminal quality" actually mean?

Bloomberg terminals don't just show charts. They provide:
- **Real-time data** — Trades appear on charts within seconds of closing
- **Cross-referencing** — Click a point on the scatter plot, see the trade detail
- **Rich tooltips** — Not just numbers, but context (was this a news trade? breakout? fade?)
- **Derived analytics** — Sharpe ratio, Sortino, rolling win rate, expectancy curves
- **Comparison** — Overlay your equity curve against a benchmark
- **Alerting** — "Your drawdown exceeded 5% this week"
- **Speed** — Sub-100ms chart loads, even with 10,000+ trades

### 4. What's actually blocking us right now?

In priority order:
1. **7 missing route handlers** — ~150 lines of Rust to wire services → HTTP
2. **JWT auth flow** — The journal app needs a login page or token injection mechanism
3. **Seed data** — No trades exist in journal_trades yet. We need either:
   - A CSV import function
   - Live trading to generate data
   - A seed script that inserts synthetic trades
4. **The Overview page** — Also calls fetchOverview(), which needs the same route wiring

### 5. What would a Bloomberg-grade journal actually look like?

**Phase 1: Data Flows (Current blocker)**
- Wire the 7 analytics endpoints
- Add JWT login or dev-mode token bypass
- Seed 100+ synthetic trades for development

**Phase 2: Analytical Depth**
- Rolling metrics (7d, 30d, 90d win rate / expectancy)
- Sharpe ratio calculation
- Equity curve with benchmark overlay (SPY, BTC)
- Calendar heatmap (GitHub-style contribution graph for trading days)
- Risk-of-ruin Monte Carlo simulation

**Phase 3: Interactivity**
- Click chart point → navigate to trade detail
- Brush-select time range on equity curve → filter all other charts
- Tag-based filtering (show only "breakout" trades vs "fade" trades)
- Annotation layer on equity curve (mark events, drawdown periods)

**Phase 4: Real-time**
- WebSocket push when new trade closes → auto-update all charts
- Live P&L streaming during open positions
- Notification system for drawdown alerts

---

## Immediate Fix: The 7 Missing Routes

### What exists (backend services):

```rust
// journal_stats.rs — StatsEngine
pub fn account_overview(&self) → AccountOverview
pub fn performance_stats(&self) → PerformanceStats
pub fn risk_stats(&self) → RiskStats

// journal_timeseries.rs — TimeSeriesService
pub fn equity_curve(&self) → Vec<EquityCurvePoint>
pub fn daily_pnl(&self) → Vec<DailyPnlPoint>
pub fn symbol_breakdown(&self) → Vec<SymbolBreakdown>
pub fn duration_profit(&self) → Vec<DurationProfitPoint>
pub fn return_distribution(&self) → Vec<ReturnBucket>
pub fn time_distribution(&self) → Vec<TimeDistribution>
```

### What's needed (HTTP handlers in journal.rs):

7 async functions, each following this pattern:
1. Extract user_id from JWT
2. Parse query params into StatsFilter
3. Call the relevant service method
4. Serialize to JSON with `{ data: [...] }` wrapper
5. Return 200

### Route registration (main.rs, after line 903):

```rust
.route("/analytics/overview", web::get().to(journal::overview))
.route("/analytics/equity-curve", web::get().to(journal::equity_curve))
.route("/analytics/daily-pnl", web::get().to(journal::daily_pnl))
.route("/analytics/symbol-breakdown", web::get().to(journal::symbol_breakdown))
.route("/analytics/duration-profit", web::get().to(journal::duration_profit))
.route("/analytics/return-distribution", web::get().to(journal::return_distribution))
.route("/analytics/time-distribution", web::get().to(journal::time_distribution))
```

---

## Auth Question

The journal frontend reads `localStorage.getItem('testudo_token')`. This token comes from:
- `testudo-web` login flow → sets token in localStorage
- But testudo-journal runs on port 3002 (different origin than testudo-web)
- LocalStorage is origin-scoped → **the token from testudo-web won't be available on port 3002**

Options:
1. **Shared auth** — Run journal under the same origin as testudo-web (subdirectory or same port)
2. **Dev bypass** — Skip auth in dev mode (dangerous but fast for testing)
3. **Token relay** — Journal login page that authenticates against the same backend
4. **Cookie-based** — Switch from Bearer token to httpOnly cookies (same-site)

---

## Verdict

The ECharts migration (UXP-16) is complete. The "blank charts" are not a charting problem — they're a **data pipeline last-mile problem**. The backend services are built, the frontend consumers are built, but the HTTP routes connecting them don't exist yet.

**Next spec needed: JNL-12 — Wire Analytics API Endpoints**
- 7 route handlers in journal.rs (~150 lines)
- 7 route registrations in main.rs (~7 lines)
- Auth token flow for the journal app
- Seed data mechanism for development

Once those routes exist, the ECharts charts will light up immediately — no frontend changes needed.
