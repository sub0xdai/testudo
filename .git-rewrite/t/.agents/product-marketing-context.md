# Product Marketing Context

*Last updated: 2026-04-10*

## Product Overview
**One-liner:** Automated risk management for crypto traders who use TradingView.

**What it does:** Testudo sits between you and your exchange, enforcing position sizing rules on every trade. You draw your setup on TradingView, press Alt+X, and Testudo calculates exact position size, places bracket orders (entry + SL + TP), and logs every trade automatically. Review performance on the Desk dashboard.

**Product category:** Trading risk management / Trade execution overlay

**Product type:** SaaS (browser extension + web dashboard + backend API)

**Business model:** $20/mo subscription OR $50 one-time lifetime "Early Legionary" pass. Token launch planned.

## Target Audience
**Target companies:** N/A (B2C — individual retail traders)

**Decision-makers:** The trader themselves

**Primary use case:** Automated position sizing and trade journaling for crypto perpetual futures

**Jobs to be done:**
- Calculate correct position size for every trade based on risk rules (so I don't blow up my account)
- Execute trades from TradingView without switching to exchange UI (so I don't miss entries)
- Track performance with real data — equity curves, R-multiples, expectancy (so I improve over time)

**Use cases:**
- Day trading crypto futures with systematic risk limits
- Journaling trades with thesis-first approach and post-trade review
- Multi-exchange management from a single interface (Hyperliquid, Binance, WOO, Bybit, OKX)

## Problems & Pain Points
**Core problem:** Most traders don't lose because of bad entries — they lose because of bad sizing. Calculating position sizes by hand is slow, error-prone, and the first thing traders skip when in a rush.

**Why alternatives fall short:**
- Manual sizing: Too slow, easily skipped, math errors under pressure
- Exchange UIs: No risk-based position sizing, no bracket order automation from TradingView
- Trading bots/algos: Generate signals (not what traders want) — traders want to keep their edge but automate the discipline
- Spreadsheet journals: Manual data entry, no real-time trade import, no analytics

**What it costs them:** Account blowups from oversizing. Missed trades from slow execution. No performance data to improve from. Emotional decision-making during losing streaks.

**Emotional tension:** "After the third consecutive loss, something changes. The stop gets widened. The size gets doubled. The system gets abandoned." Fear of ruin. Frustration at repeating the same mistakes. Doubt about whether their edge is real.

## Competitive Landscape
**Direct:** No direct competitor does all three (risk-based sizing + TradingView execution + automated journaling) in one product for crypto futures.

**Secondary:** TradeZella, Tradervue, Edgewonk — trade journaling only, no execution, no automated sizing. Manual data import.

**Indirect:** Exchange built-in tools (Binance/Bybit position calculators) — basic, no automation, no cross-exchange, no journaling. TradingView alerts → webhook bots — fragile, no risk engine, no journal.

## Differentiation
**Key differentiators:**
- Alt+X hotkey: One keystroke from TradingView chart to sized, bracketed order on exchange
- Conservative-wins sizing: MIN(account%, fixed risk, max size, margin capacity) — the most conservative constraint always wins
- Automated journal pipeline: Every trade logged with full lifecycle data (entry, fills, SL/TP, duration, R-multiple) — zero manual entry
- Multi-exchange from one interface: Hyperliquid (DEX) + Binance, WOO, Bybit, OKX (CEX)
- Dignitas score: Composite trading performance rating (radar chart)

**How we do it differently:** Testudo is not a bot. It doesn't pick entries. You decide when and where to trade. Testudo makes sure you size it correctly and keeps a record.

**Why that's better:** Traders keep their edge and their judgment. They just get protected from the version of themselves that abandons the plan at 2am.

**Why customers choose us:** Speed (Alt+X), discipline enforcement (automated sizing), and the journal pipeline (no manual logging).

## Objections
| Objection | Response |
|-----------|----------|
| "I can calculate position size myself" | You can. But will you at 2am on your third consecutive loss? Automation protects you from the version of yourself that skips the math. |
| "I don't want a bot trading for me" | Testudo is not a bot. It doesn't generate signals. You decide every trade. It just sizes and manages the execution. |
| "What about security / my API keys?" | AES-256-GCM encryption at rest. Trading permissions only — never withdrawal access. Hyperliquid uses agent wallets (valet key model). |
| "$20/mo for a position calculator?" | It's sizing + execution + journaling + analytics. The $50 lifetime deal exists for early adopters who see the value immediately. |

**Anti-persona:** Algo/quant traders who want fully automated signal-to-execution pipelines. Spot traders (no leverage). Traders who don't use TradingView. People looking for trade signals or "what to buy."

## Switching Dynamics
**Push:** Manual sizing mistakes. Slow execution switching between TradingView and exchange. No performance data. Emotional overrides during drawdowns.

**Pull:** One-keystroke execution. Automatic risk enforcement. Real performance data (not feelings). The journal forces honest self-review.

**Habit:** Familiar exchange UIs. Existing spreadsheet journals. "I've always done it this way."

**Anxiety:** Trusting a third party with API keys. Learning a new tool. "Will it work with my exchange?"

## Customer Language
**How they describe the problem:**
- "I keep oversizing my trades"
- "I know I should journal but I never do"
- "I can't stop myself from revenge trading"
- "I blew up my account again"
- "I skip the position size calculation when I'm in a rush"
- 
**How they describe us:**
- "It's like having a risk manager sitting next to you"
- "The Alt+X thing is addictive — I can't go back to manual"
- "The journal actually shows me what I'm doing wrong"
- 
**Words to use:** Risk management, position sizing, discipline, formation, shield wall, R-multiple, expectancy, edge, survive, outlast

**Words to avoid:** Bot, algo, automated trading (implies signal generation), AI trading, copy trading, social trading, guaranteed returns

**Glossary:**
| Term | Meaning |
|------|---------|
| Testudo | Roman shield formation (tortoise). The product name. |
| Alt+X | Hotkey to trigger trade from TradingView |
| R-multiple | How much you made relative to what you risked |
| Expectancy | Average R per trade — the single number that says if your system works |
| Dignitas | Composite performance score (radar chart on desk) |
| The Desk | Web dashboard at /desk for performance review |
| The Formation | Brand philosophy — discipline over prediction |
| Conservative wins | Sizing rule: take the smallest of all constraints |
| Bracket order | Entry + stop-loss + take-profit placed together |

## Brand Voice

**Tone:** Direct, confident, slightly militant. No fluff. Roman gravitas.

**Style:** Short declarative sentences. Monospace aesthetic. Technical but accessible. Second person ("you").

**Personality:** Disciplined, stoic, protective, no-nonsense, brutalist.

## Proof Points

**Metrics:** 972 passing tests, 160+ commits, 537 total commits across 3 months. Multi-exchange verified (WOO, Hyperliquid, Binance).

**Customers:** Solo dev product, pre-launch. Early Legionary lifetime passes available.

**Testimonials:** N/A (pre-launch)

**Value themes:**
| Theme | Proof |
|-------|-------|
| Discipline over prediction | "You bring the edge. We enforce the discipline." |
| Sizing is the edge | "A trader with a 40% win rate and proper sizing will outlast one with 80% who sizes recklessly." |
| Automation protects you from yourself | "The version that writes the plan on Sunday evening vs. the version that abandons it on Tuesday at 2am." |
| Data over feelings | "The patterns in your data will tell you truths your ego never will." |

## Goals

**Business goal:** Launch to market. Acquire first 100 paying users. Prove product-market fit.

**Conversion action:** Connect wallet → Add exchange → Execute first trade via Alt+X

**Current metrics:** Pre-launch. Solo dev. Product functional and deployed.
