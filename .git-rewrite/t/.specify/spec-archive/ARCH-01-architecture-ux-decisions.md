# Architecture & UX Decision Log

**Date:** 2026-03-26
**Status:** Living Document
**Source:** Grill-me session — systematic interrogation of architecture and UX decisions

---

## Decisions Made

### D1: Three-App Architecture (Confirmed)

**Landing page (testudo-web)** — marketing, docs, info. Pure static site.
**Desk (testudo-journal)** — authenticated dApp. Analytics, journal, account management.
**Extension (testudo-extension)** — core product. TradingView sniper + trade execution.

The extension is the primary product. The dApp is deep-dive support infrastructure. The landing page is the front door.

### D2: Migrate Landing Page to Astro

React is accidental, not intentional. No React-specific components required. DESK-02 already strips Web3 from testudo-web.

**Decision:** Rewrite testudo-web as an Astro static site.
- Zero JS by default, component islands for interactivity (theme toggle)
- Solid.js components via Astro islands for consistency
- First-class content collections for docs and blog
- Near-zero bundle, perfect SEO, faster first paint
- Solves the docs hosting question simultaneously

### D3: Docs via Astro Content Collections

No separate docs framework (MkDocs/GitBook). Docs live as markdown content collections within the Astro landing site. Professional appearance, single deployment, no extra infrastructure.

### D4: Exchange History Import (New Feature)

The dashboard currently only shows trades placed through the extension. A wallet-connected user with exchange API keys sees empty charts — a retention cliff.

**Decision:** Import trade history from connected exchanges.
- Use safe-cex library + Hyperliquid SDK (same exchanges already supported)
- Import up to 90 days of history on exchange connection
- Optional feature, not required
- Imported trades render identically to extension-placed trades (no special badges)
- `journal_trades` schema fields like `trade_group_id`, `risk_amount` will be NULL for imported trades

**Impact:** Users see value at step 3 of onboarding (before extension install). Dashboard is immediately useful.

### D5: Numbered Onboarding Flow

Current UX presents wallet connect and extension install as interchangeable entry points. In reality, the path is strictly sequential.

**Decision:** Implement explicit numbered onboarding in the dApp.

```
Step 1: Connect wallet (SIWE on dApp)
Step 2: Add exchange API keys (account page)
Step 3: Exchange history imports → dashboard populates
Step 4: Install extension (link to Chrome/Firefox store) ← NAGGED
Step 5: Pair extension (6-digit code)
```

- Onboarding UI hides after completion but can be revisited
- Extension install step nags persistently — there is no reason not to install it
- Extension becomes an upgrade ("trade directly from TradingView") not a prerequisite
- Split CTA on landing page: "INSTALL EXTENSION" (primary) + "LAUNCH DESK" (secondary)

### D6: Extension Stays Lean

No onboarding wizard in the extension popup (360×600px is too tight). A `?` help icon with dropdown/modal pointing to the dApp handles guidance. Demo mode with fake data is overengineering — solved with instruction videos and blog posts instead.

### D7: Overview Page — Equity Curve Forward

The overview is a "wall of stats at a glance" with the equity curve as the hero element. Account stats, performance stats, and risk stats in sidebars — all equal weight, no hidden panels.

### D8: Journal is Optional Manual Diary

- Manual diary entries with markdown
- Screenshot/image support
- Export to `.md` for local storage
- Not auto-generated, not required
- Separate purpose from the Trades tab

### D9: Journal Entry from Trade Row

Trades tab gets a "journal" action per trade row. Clicking opens the journal editor with trade metadata pre-linked. Trades with existing journal entries show a visual indicator (icon/badge) so users can see which trades they've reflected on.

**Open:** Whether the journal editor opens as a slide-out panel, modal, or inline expansion. TradeDetail already slides from the right — journal entry could live within that panel.

### D10: Trade Confirmation — No Extra Friction

Double-Enter safety on the Alt+X modal is sufficient. No countdown timers, no symbol-typing, no additional gates. Retail day traders move fast.

### D11: Scraper Failure Feedback

When Alt+X fails (wrong page, no position tool, unsupported pair), the user gets a toast notification with the reason (e.g., "Pair not supported"). No silent failures.

---

## Open Decisions

### O1: Monetization Model

Leaning toward: **Subscription first, token later.**
- Tiered subscription for desk access
- Build user base first
- Token launch later with separate utility (not access-gating the core product)
- Hybrid (token + sub) not ruled out

**Unresolved:**
- Pricing tiers and what each unlocks
- Payment method (Stripe? crypto payments? both?)
- Whether the extension is free or gated
- Token utility if/when launched

### O2: Target User Profile

**Confirmed:** Retail day traders who use TradingView.

**Unresolved:**
- Which TradingView tier (free users? Pro? Premium with DOM?)
- Geographic focus (US regulatory implications?)
- Experience level (pro traders? learning traders?)

### O3: Journal Entry UX Details

**Unresolved:**
- Auto-populate journal with trade metadata template, or blank canvas with trade linked as metadata?
- Per-entry export or bulk export?
- Does exported markdown include trade metadata or just notes?
- Screenshot storage: S3, PostgreSQL blob, or base64?
- If PostgreSQL — are traders comfortable with strategy notes on a remote server?

### O4: Framework Migration Path

**Decided:** Astro for landing page.
**Unresolved:**
- When to execute the migration (before or after DESK-02?)
- Does DESK-02 become "migrate to Astro" instead of "strip Web3 from React"?
- Astro theme: port existing brutalist dark aesthetic or redesign?

---

## Architecture Insights Surfaced

1. **The four-step funnel problem.** Without history import, users must: connect wallet → add API keys → pair extension → place first trade before seeing any value. Every step is a drop-off point. History import moves value delivery to step 3.

2. **The dashboard isn't useless without the extension — it's useless without data.** Exchange API keys already exist for balance/order placement. Extending them to pull history is the natural completion of that integration.

3. **Extension and dApp are two modes of one product.** Extension = execution speed. dApp = reflection and analysis. Not primary/secondary — complementary. Marketing should frame it this way.

4. **The empty dashboard is a retention cliff.** Before history import exists, consider placeholder/demo data to show what the dashboard looks like populated. Not fake data in production — but an onboarding preview state.
