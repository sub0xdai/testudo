# Implementation Plan

> Last updated: 2026-03-27
> Current spec: DOCS-01-comprehensive-documentation
> Phase: COMPLETE

---

## Active Spec: DOCS-01-comprehensive-documentation

Write all 10 documentation pages for the Testudo docs site. Infrastructure (content collection, layouts, sidebar, routing, KaTeX) already built. Each task writes one doc file.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Write `01-what-is-testudo.md` — problem, solution, components, audience | complete | low | — |
| T2 | Write `02-core-concepts.md` — position sizing, R-multiples, expectancy, profit factor, drawdown, win rate vs edge (KaTeX formulas) | complete | high | — |
| T3 | Write `03-getting-started.md` — wallet connect, add exchange, import history, install extension, pair | complete | medium | — |
| T4 | Write `04-extension.md` — Alt+X workflow, scraper, modal, double-Enter, popup overview | complete | medium | — |
| T5 | Write `05-dashboard.md` — equity curve, stats sidebar, chart selectors, filtering, time presets | complete | medium | — |
| T6 | Write `06-journal.md` — thesis-first approach, notes, tags, timeline, collections, export | complete | medium | — |
| T7 | Write `07-exchanges.md` — per-exchange setup (HL, WOO, Binance, Bybit, OKX), API key guides | complete | medium | — |
| T8 | Write `08-faq.md` — common issues and troubleshooting | complete | low | — |
| T9 | Write `09-architecture.md` — system diagram, component map, data flow | complete | medium | — |
| T10 | Write `10-api-reference.md` — REST endpoints, WebSocket, auth flow | complete | high | — |
| T11 | Verify `bun run build` passes in testudo-web | complete | low | T1–T10 |

### Key Decisions

- **Audience-first**: Trader docs (1–8) use plain English with concrete examples. Technical docs (9–10) assume engineering context.
- **KaTeX for math**: Position sizing formula, expectancy, R-multiples use block/inline math notation.
- **No screenshots yet**: Text-only first pass. Screenshots can be added in a follow-up.
- **Accurate to codebase**: All descriptions verified against actual source code via research agents.

### Discoveries

(populated during build iterations)

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| HIST-01-exchange-history-import | 2026-03-26 |
| ONBOARD-01-stepper-onboarding | 2026-03-26 |
| DESK-02-landing-strip | 2026-03-26 |
| EXT-41-desk-dashboard | 2026-03-24 |
| EXT-40-smart-card-grid | 2026-03-24 |
| EXT-39-pair-ux | 2026-03-24 |
| AUTH-03-frontend-auth | 2026-03-24 |
| AUTH-02-backend-auth | 2026-03-24 |
| AUTH-01-infra-hardening | 2026-03-24 |
| ANL-01-bloomberg-charts (Phase 1) | 2026-03-23 |
| JNL-18-storage-quotas | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| UXP-18-multi-theme | 2026-03-21 |
| HL-11-status-transition-fix | 2026-03-21 |

---

*This file is persistent state. Vox updates it each iteration.*
