# Specification: Comprehensive Documentation

**Spec ID:** DOCS-01-comprehensive-documentation
**Date:** 2026-03-27
**Status:** Draft
**Class:** Documentation
**Priority:** P1 — No user-facing documentation exists. Traders cannot learn how to use the product without it.
**Depends on:** DESK-02-landing-strip (Astro infrastructure)
**Series:** DOCS-01 (standalone)

---

## Problem Statement

Testudo has no documentation. A trader who discovers the product has no way to understand what it does, how position sizing works, or how to set it up — beyond reading the About page manifesto. The Astro landing site supports content collections but has no docs pages.

The documentation must serve two audiences: retail crypto traders (primary) who need to understand the trading concepts and workflow, and technical users who want to understand the architecture.

---

## Content Structure

### Part I: For Traders

#### 1. What is Testudo?
- The problem: most traders lose from poor risk management, not bad entries
- What Testudo does: automated position sizing + trade journal
- The three components: Extension (execution), Desk (analytics), Exchange connections
- Who it's for: retail day traders on TradingView trading crypto perps

#### 2. Core Concepts
Mix technical accuracy with plain-English explanations. Each concept gets:
- **What it is** (one sentence)
- **Why it matters** (trader context)
- **The math** (formula in LaTeX/KaTeX notation)
- **Example** (concrete numbers)

Concepts to cover:

**Position Sizing**
- Fixed fractional: risk X% of account per trade
- Formula: `position_size = (account_balance × risk_percent) / stop_distance`
- Example: $10,000 account, 1% risk, $500 stop distance → 0.2 BTC
- Why: a 40% win rate with proper sizing beats 80% win rate with reckless sizing

**R-Multiples**
- Definition: reward measured in units of risk
- Formula: `R = net_pnl / risk_amount`
- Example: risked $100, made $250 → 2.5R. Lost $100 → -1R
- Why: normalizes wins/losses across different position sizes

**Expectancy**
- Definition: average R per trade across your history
- Formula: `E = (win_rate × avg_win_R) - (loss_rate × avg_loss_R)`
- Example: 40% wins at 2R, 60% losses at 1R → E = 0.8 - 0.6 = 0.2R
- Why: positive expectancy = profitable system over time

**Profit Factor**
- Formula: `PF = gross_profit / gross_loss`
- PF > 1 = profitable, PF > 2 = strong edge

**Maximum Drawdown**
- What it is: largest peak-to-trough decline in account equity
- Why: determines if you can psychologically survive your system's losing streaks

**Win Rate vs Edge**
- The counterintuitive truth: win rate alone means nothing
- A 30% win rate with 3:1 R is better than 70% win rate with 0.3:1 R
- Table showing different win rate + R-multiple combinations and their expectancy

#### 3. Getting Started
Step-by-step setup:
1. Connect wallet (SIWE on the Desk)
2. Add exchange API keys (WOO, Binance, Bybit, OKX, Hyperliquid)
3. Import trade history (automatic on exchange connect)
4. Install browser extension
5. Pair extension with 6-digit code
- Screenshots/diagrams for each step

#### 4. The Extension
- Installing from Chrome Web Store
- The Alt+X workflow on TradingView
  - Open position tool on chart
  - Press Alt+X
  - Confirm in modal (entry, SL, TP, position size auto-calculated)
  - Double-Enter safety mechanism
- Reading the popup: balance, exposure gauge, active positions
- CEX vs DEX mode

#### 5. The Desk Dashboard
- Overview: equity curve, P&L stats, risk metrics
- Journal: trade table, detail sidebar, notes, tags
- Writing a thesis (pre-trade notes on active positions)
- Filtering: time presets, exchange, symbol, side, tags
- Account: exchange management, extension pairing

#### 6. The Journal Workflow
- The thesis-first approach: write why before you trade
- During-trade notes: documenting management decisions
- Post-trade review: what happened vs what you expected
- Tags for categorization (scalp, trend-follow, reversal, etc.)
- Markdown export (.md with YAML frontmatter)
- Building a track record over time

#### 7. Exchange Setup Guides
Per-exchange instructions:
- **Hyperliquid**: Connect wallet → agent wallet → approve
- **WOO**: API key generation (futures permissions only)
- **Binance**: API key with futures trading enabled, IP whitelist
- **Bybit**: API key with derivatives permissions
- **OKX**: API key + passphrase

#### 8. FAQ & Troubleshooting
- "My trade isn't showing on the dashboard" → journal shows closed trades
- "The extension says 'pair not found'" → generate new code
- "Position sizing seems wrong" → check risk config
- "Import didn't work" → check API key permissions

### Part II: Technical

#### 9. Architecture Overview
- System diagram: Extension → Backend → Exchange
- Component map: testudo-exchange (Rust), testudo-extension (Solid.js), testudo-journal (Solid.js), testudo-web (Astro)
- Data flow: TradingView DOM scrape → REST API → Shadow Engine → Exchange

#### 10. API Reference
- Authentication: SIWE + JWT + HttpOnly cookies
- Key endpoints: /trades, /journal/*, /exchanges/*
- WebSocket: order.{user_id} channel

---

## Technical Implementation

### Astro Content Collections

```
testudo-web/src/
├── content/
│   ├── config.ts
│   └── docs/
│       ├── 01-what-is-testudo.md
│       ├── 02-core-concepts.md
│       ├── 03-getting-started.md
│       ├── 04-extension.md
│       ├── 05-dashboard.md
│       ├── 06-journal.md
│       ├── 07-exchanges.md
│       ├── 08-faq.md
│       ├── 09-architecture.md
│       └── 10-api-reference.md
├── pages/
│   └── docs/
│       ├── index.astro (docs landing / table of contents)
│       └── [...slug].astro (dynamic doc pages)
├── components/
│   ├── DocsSidebar.astro (navigation)
│   └── DocsLayout.astro (sidebar + content + prev/next)
└── styles/
    └── docs.css (prose styling, code blocks, math)
```

### Content Collection Schema

```typescript
// src/content/config.ts
import { defineCollection, z } from 'astro:content'

const docs = defineCollection({
  type: 'content',
  schema: z.object({
    title: z.string(),
    description: z.string(),
    order: z.number(),
    section: z.enum(['trader', 'technical']),
  }),
})

export const collections = { docs }
```

### Math Rendering

Use KaTeX for math formulas in markdown:
- Install `remark-math` + `rehype-katex`
- Configure in `astro.config.mjs`
- Inline math: `$E = W \cdot \bar{R}_w - L \cdot \bar{R}_l$`
- Block math: `$$\text{size} = \frac{\text{balance} \times \text{risk\%}}{\text{stop distance}}$$`

### Prose Styling

Use Tailwind typography plugin (`@tailwindcss/typography`) for clean markdown rendering. Override with brutalist dark theme tokens.

### Dependencies

- `@astrojs/mdx` — MDX support for interactive examples
- `remark-math` — parse math notation in markdown
- `rehype-katex` — render KaTeX
- `@tailwindcss/typography` — prose styling

---

## Acceptance Criteria

- [ ] 10 documentation pages covering all sections
- [ ] Math formulas render correctly (KaTeX)
- [ ] Sidebar navigation with section grouping
- [ ] Prev/next links between pages
- [ ] Mobile-responsive docs layout
- [ ] Dark/light theme support
- [ ] `/docs` route accessible from landing page header
- [ ] `bun run build` passes for testudo-web

---

## Completion Signal

This spec is complete when:
1. All 10 documentation pages are written and render correctly
2. Math formulas display properly
3. Navigation works (sidebar + prev/next)
4. Accessible from landing page
5. `bun run build` passes
6. Code committed to master
