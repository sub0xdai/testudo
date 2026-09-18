# Plan: TypeSafe Jev x Testudo Sniper Integration - Technical Proposal

**Status:** Design review - no implementation code written. Awaiting answers to Section 9.
**Date:** 2026-06-05 (research current at time of writing)
**Targets inspected:** `testudo-extension/`, `testudo-journal/`, `testudo-exchange/`
**Goal:** Add TypeSafe's Jev System One model to the Testudo browser extension as a source of fast, structured decisions that produce no generated text.

---

## 1. Research Findings

### 1.1 Correction on "Testudo Sniper"

I could not find a publicly documented "Testudo Sniper" Chrome extension, and this should be settled before acting on the brief.

Two web searches returned no such product.
What the web surfaces under "Testudo" is an unrelated Ethereum wallet-security extension (transaction interception, EIP-7702 delegation warnings) in a different repository.
The trading-related hits are third-party tools: Sniperoo, SniperX, Axiom Sniper, Trading Power Tool, Lumina Position Sizer.
Nothing indexes `testudo.vip` or an Alt+X TradingView overlay.

The name exists only as internal build artifacts in this repo:

| Artifact | Value |
|---|---|
| `testudo-extension/build.ts:159` | Firefox add-on ID `testudo-sniper@sub0xdai` |
| `testudo-extension/testudo-sniper-chrome.tar.gz` | 546 KB, Mar 29 |
| `testudo-extension/testudo-sniper-firefox-1.1.5.zip` | 204 KB, May 1 |
| `manifest.json` name / `store-assets/store-listing-copy.md` | published as **"Testudo"**, not "Testudo Sniper" |

The target of this proposal is the in-repo extension at version 1.1.5, published under the name "Testudo".
If a separate Sniper build or fork exists that I have not seen, say so and I will re-scope.
Everything below is derived from the code in `testudo-extension/`, not from a store listing.

### 1.2 What the extension actually does

Single flow, one hotkey.

| Step | Where |
|---|---|
| User draws a Long/Short Position tool on a chart | TradingView, DexScreener, GMX, Hyperliquid, or an embedded chart on Bybit/Binance/OKX/Bitget/Gate/Phemex/BloFin |
| Alt+X (or the `trigger-trade` browser command) fires | `src/content.ts:252`, `src/background.ts:30` |
| Scraper extracts the setup | `src/scraper.ts` - 6 fallback strategies, bridge into `window.TradingViewApi.activeChart()` reading `getAllShapes()` plus `stopLevel`/`profitLevel` scaled by tick size |
| Modal opens pre-filled | `src/modal.tsx` to `src/components/TradeForm.tsx` (671 lines) |
| Live sizing preview | `POST /api/v1/trades/preview` returning `SizingPreview` |
| Double-Enter confirms | `onConfirm` to `EXECUTE_TRADE` to `POST /api/v1/trades` |
| Journaled automatically on close | server-side |

Data the extension holds at the Alt+X moment:

- symbol, side, entry, stop, target, timeframe
- a free-text `setup_tag`
- which fields the scraper auto-filled (`autoFilledFields`)
- the live balance
- the active exchange account name
- the management preset
- a 20-entry scraper health history

Auth is a paired JWT in `browser.storage.local`, refreshed via `POST /api/v1/auth/extension-refresh`.
There is currently no third-party API key anywhere in the extension, and no service-worker-to-third-party call path.

### 1.3 TypeSafe Jev - current API facts

Source: `docs.typesafe.ai`.

**Endpoint**

| | |
|---|---|
| `POST https://api.typesafe.ai/v1/systemone` | `Authorization: Bearer <API_KEY>` |
| Request | `state` (string \| object \| array), `model`, `questions` (map of id to Question) |
| Response | `model`, `answers` (map of id to Answer), `usage.input_tokens`, `usage.output_tokens` |

**Three primitives**

| Primitive | Input | Output |
|---|---|---|
| `noul` | `instructions`, optional `criteria` `{true, false}` | `noul` float 0..1 |
| `choice` | `instructions`, `criteria` map of option to rubric or null | `choice`, `probabilities`, `confidence` |
| `score` | `instructions`, `criteria` ordered array (minimum 2 levels) | `score`, `legend`, `probabilities`, `confidence` |

**Limits and cost (Jev 1.13 / `jev-1.13.0`)**

| Constraint | Value |
|---|---|
| Rate limit | **1,200 requests per minute** and 250,000 tokens per second, account-wide |
| Context | 64k tokens per request; 32k for `state` plus the longest question |
| Input | Text only - no image, audio, or video |
| Price | $42 per Btok input; **output tokens free** |
| Errors | `401`, `422`, `429 Too Many Requests`, `529 Overloaded`. Exponential backoff, honor `retry-after` |
| Aliases | `jev-latest` and `jev-preview` both currently resolve to `jev-1.13.0` |

**Mechanics that matter for the design**

- Every question in one request sees the same state, is evaluated independently and in parallel.
- Adding questions barely changes latency. Output is free, so extra questions cost only input tokens.
- Question IDs are for your code and are **not sent to the model**. The full meaning must live in `instructions`.
- `confidence` is distribution concentration, not correctness, and explicitly **not permission to act**.
- No Rust SDK exists. `@typesafe-ai/sdk` (JS) and `typesafe_sdk` (Python) do.
- The batch win is measured, not theoretical. The docs' parallel-questions cookbook reports batching 13 questions into one call as roughly 10x faster and 12x cheaper than 13 separate calls, with no change in the answers.

### 1.4 Relevant prior art in this repo

Three things already exist that shape the plan.

1. **The router is already a trusted secret boundary.** `OPENAI_API_KEY` is read server-side in `crates/router/src/main.rs:414` for the coach narrator, with an explicit warning path when unset. Adding a TypeSafe key is the same pattern, not a new one.
2. **`reqwest` 0.12 is already a direct dependency** of `crates/router/Cargo.toml:35`. A TypeSafe HTTP client needs zero new crates.
3. **A calibrated-risk pipeline is already live and already keyed on free text.** `SizingPreview` returns `edge_multiplier` plus a tagged `SizingReasoning` union (`calibrated` / `untagged` / `negative_edge` / `fixed_mode`), and the client already renders `CalibrationReasoning`. `services/calibration.rs` loads `SetupStats` and blends them with the user's global prior via Bayesian shrinkage at pseudocount K.

---

## 2. Constraints That Shape the Design

Four, and they eliminate most of the obvious ideas.

1. **Jev cannot see the chart.** Text only. No screenshot, no candles, no image of the drawing. Any use case that needs visual market structure is out, and should stay in code, which already has klines, ticker, depth, and the shadow engine.
2. **Jev cannot price or predict.** It is not trained on markets and is not fine-tuned per account. Every judgement must be a semantic or textual one that a knowledgeable person answers in about a second.
3. **The money path must stay deterministic.** `edge_multiplier` is derived from the user's own realized R-multiples. Jev must never become an input to Kelly sizing. It may only help label trades, and labels then feed calibration through realized outcomes. This keeps sizing evidence-based.
4. **The extension's text surface is thin today.** It reads geometry and numbers. The only free text at Alt+X is `setup_tag`. Any real use case requires capturing one more text field that the scraper currently discards.

Constraint 3 is the important one.
The value here is not "AI decides the trade."
It is "free text stops leaking out of the system, so the existing deterministic machinery gets better inputs."

---

## 3. Use Cases

Three, each a fast structured decision with no generated text.
Ranked by value and by how safe the failure mode is.

### UC-1 - Setup tag resolution (Choice)

**Problem.** `services/calibration.rs` matches historical setups with `LOWER(setup_tag) = LOWER($2)`, which is exact string equality.
The modal offers suggestions from `GET /api/v1/journal/setup-tags`, but a user who types `brkout`, `BO retest`, or `breakout retest` gets zero matches, silently falls into `SizingReasoning::Untagged`, and loses the calibrated Kelly path entirely.
This is a real, silent degradation of the platform's core feature, caused by a string mismatch.

**Judgement.** One Choice question.
Code supplies candidates via prefix and fuzzy match over the user's own tag list, always appending `new_tag` and `no_match`.
Jev selects the intended one.

**Why Choice.** The answer set is closed, and the model cannot invent a tag that is not offered. The docs are explicit that the model cannot choose an omitted value.

**What code does.** Uses the selected tag verbatim for `setup_tag` so calibration matches. `new_tag` keeps the user's literal text. `no_match` leaves the field empty.

**Why this is safe.** It is a preference selection among the user's own labels.
The docs note that low confidence does not invalidate a harmless preference choice.
An error costs a calibration lookup, never money.

**Why this is the strongest case.** It is the "select instead of generate" pattern.
The state is tiny, the closed set comes from existing data, and the payoff lands directly on the calibrated sizing path.

### UC-2 - Drawing-versus-intent consistency check (Noul)

**Problem.** The scraper has six fallback strategies and a health history because reads fail.
Failure modes include picking the wrong shape when several are drawn, and deriving a side that does not match what the user meant.
The current UI cannot tell the difference between "the scraper read your drawing correctly" and "the scraper invented `entry: 0`".
A side flip here places a trade in the wrong direction.

**Judgement.** One Noul question.
State carries the position tool's own text label (from `getProperties().text`, which the scraper reads today only for `stopLevel` and `profitLevel`), the scraped geometry, the side, the derived R:R, and the scrape health context.
The question: does the drawing's own annotation agree with the side and direction being sent.

**What code does.** On a high-probability mismatch, insert one extra confirmation step in the modal naming the specific disagreement.
It never mutates the trade and never silently corrects a field.

**Deliberate limits.** This must be an advisory gate, not an automated veto.
Confidence is distribution concentration, not correctness, and a false positive that blocks a legitimate trade is worse than the misread it prevents.
Log every verdict to `trade_events` so the check itself can be evaluated against realized mistakes before it is ever allowed to be firmer.

### UC-3 - Journal note attribution (Choice + Noul + Score, one batched request)

**Problem.** Journal notes live in `journal_entries.notes` as unstructured text.
Dignitas already weights `weight_journal_consistency` for note *presence*, but nothing reads note *content*.
The coach's seven pattern detectors (`frequency_spike`, `session_anomaly`, `setup_fatigue`, `sizing_drift`, `streak_risk`, `correlation_stack`) operate on numbers only, so a trader's own written self-report contributes nothing to their analytics.

**Judgement.** One request, three questions over the same state, evaluated in parallel:

- **Choice** - which of the user's existing tags does this note describe, if any.
- **Noul** - does the note describe a rule break or an execution error the trader is aware of.
- **Score** - was the exit planned or improvised.

**What code does.** Persists structured fields alongside the raw note.
The coach's digest gains a semantic input, and the tag Choice feeds the same calibration key as UC-1.
Thresholds are owned by code and tuned against real data.

**Why batched.** Three questions in one call rather than three calls.
Output is free and questions run in parallel, so this costs only a few hundred extra input tokens.

### Request count per trade

Two, at different moments.
UC-1 and UC-2 fire together on Alt+X (same state, one request). UC-3 fires once on close (different state, different moment).
At roughly 2k input tokens per request that is about $0.00008 per trade, and well inside the 1,200 rpm ceiling.

### Use cases considered and rejected

| Idea | Why not |
|---|---|
| "Should I take this trade?" | Needs market structure and edge estimation. Not a one-second semantic judgement, and not something Jev is trained for |
| Chart regime classification | Needs vision. Jev is text-only, and the repo already has klines and tickers for this |
| Confidence-weighted position sizing | Puts a non-deterministic signal on the money path. Violates constraint 3 |
| News sentiment as trade state | No news source is currently ingested, and adding one is a separate project |
| Auto-tagging without candidate generation | The model would generate tags, which is exactly the failure mode UC-1 exists to remove |

---

## 4. Architecture

### 4.1 Answer to the routing question

**Route every call through the existing Testudo router (`testudo-exchange`). Do not call `api.typesafe.ai` from the extension, and do not build a separate proxy service.**

The reasoning is specific to this codebase, not generic advice.

**Why not the extension.** An MV3 extension cannot hold a secret.

| Exposure | Detail |
|---|---|
| Bundle | Anything in `dist/` is readable by unzipping the CRX or opening the unpacked extension |
| Storage | `browser.storage.local` is readable from devtools and is not a secret store. The extension's own design already treats it that way. The JWT there is a short-lived, refreshable, per-user token, not a long-lived account-wide credential |
| Content scripts | `content.js` runs in an isolated world on 11 third-party origins (`tradingview.com`, `bybit.com`, `binance.com`, and others). A TypeSafe key reachable from a content script leaks through any XSS or dependency compromise on any of those pages |
| Blast radius | The key is account-wide across all Testudo users. One leak is every user's rate limit and bill |

**Why not a separate proxy.** The router already is the trust boundary.
It already terminates the paired JWT, already reads server-side secrets, and already owns per-user context.
A second service adds a hop, a deploy target, a second secret store, and a second failure mode for no gain.
The trust boundary already exists in this codebase, so use it.

**Also ruled out:** the key in `manifest.json`, in `wrangler.jsonc`, in a Cloudflare Worker bound to the extension, or in any `.env` file that ships with an extension build.

### 4.2 Placement in the router

Mirror the coach service layout, which is the existing precedent for calling an external model from the server.

| Layer | Path | Responsibility |
|---|---|---|
| Client | `crates/router/src/services/typesafe/client.rs` | `reqwest` POST to `/v1/systemone`. Uses the existing `reqwest` 0.12 dependency. Typed error union covering `429`, `529`, `401`, `422`, timeout, network |
| Types | `crates/router/src/services/typesafe/types.rs` | Request and answer types as a discriminated union on `type`, matching `noul` / `choice` / `score` |
| Service | `crates/router/src/services/typesafe/service.rs` | Assembles state from server-side data, issues questions, applies code-owned thresholds |
| Route | `crates/router/src/routes/judgment.rs` | New authenticated routes, registered in `main.rs` alongside the existing scopes |
| State | `crates/router/src/types/app.rs` | New field on `AppState`, constructed in `main.rs` the same way the narrator is |

Config: `TYPESAFE_API_KEY` from the environment, same treatment as `OPENAI_API_KEY` at `main.rs:414`, including a startup warning when unset.
Secrets ride the existing Infisical path (`.infisical.json`).

### 4.3 Proposed routes

| Route | Purpose | Auth |
|---|---|---|
| `POST /api/v1/judgment/pre-trade` | UC-1 + UC-2, one batched request | paired JWT, `auth: "hard"` |
| `POST /api/v1/judgment/post-trade` | UC-3, called internally on close | internal or paired JWT |
| `GET /api/v1/health/typesafe` | Reachability, mirrors the existing `/api/v1/health/sidecar` pattern | none |

The extension's call surface does not change in kind.
It already posts to `settings.backendUrl` through `apiRequest()` with a paired JWT.
Two new message types in `RuntimeMessageSchema` and two new handlers in `handlers.ts` is the whole client delta.

### 4.4 Failure discipline

Non-negotiable, because a judgement is a nicety and a trade is not.

| Condition | Behaviour |
|---|---|
| `429` or `529` | Exponential backoff with `retry-after`, then **skip the judgement**. Fail open |
| Timeout | Budget of roughly 500 ms on the Alt+X path, then skip. The existing non-blocking `GET_BALANCE` call in `content.ts` is the pattern to copy |
| `401` | Log loudly and disable the feature. A bad key must not be retried per request |
| Unparseable answer | Skip. Never guess |
| `TYPESAFE_API_KEY` unset | Extension behaves exactly as it does today. Degrade to a no-op, not to an error |

The `429` case matters more than it looks.
The 1,200 rpm limit is account-wide across all Testudo users, and the docs state the limits are currently adjusting dynamically.
A single runaway client loop could exhaust it for everyone.
That is an argument for the server-side placement on its own.

### 4.5 Rate and cost control

| Control | Where |
|---|---|
| Per-user token bucket, well under the account ceiling | router, before the outbound call |
| Cache setup-tag resolution on `(user_id, normalized_tag_text)`, 5-minute TTL | router. Mirrors the extension's existing 5-minute `setup_tagCache` |
| Pin `model` to the versioned `jev-1.13.0`, not `jev-latest`, once any threshold is tuned | request construction |
| Log `model`, `usage.input_tokens`, `usage.output_tokens` per call | `trade_events` or a dedicated row |

Pinning matters because aliases move without a change on your side, and the docs say to pin once thresholds are tuned against a version.

---

## 5. State Management

### 5.1 Principle

Assemble state **server-side** from data the router already owns, and treat anything the extension sends as untrusted input that is parsed into a closed type at the boundary.

This is not ceremony.
The extension is a client on hostile pages.
Everything it reports about the DOM is attacker-influenced text on a page the user does not control, and that text is about to be fed to a model whose output influences UI and labels.
The extension already does boundary parsing properly with zod (`schemas.ts`, `RuntimeMessageSchema` is a discriminated union), so this stays consistent with the existing code.

### 5.2 Data the extension already captures

| Field | Origin | Notes |
|---|---|---|
| `symbol`, `side`, `entry`, `stop`, `target`, `timeframe` | `scraper.ts` bridge or DOM strategy | numbers plus two closed strings |
| `scraped_vs_manual` | `autoFilledFields` set in `TradeForm.tsx` | tells you whether a human corrected the scrape |
| `setup_tag` | user-typed in the modal | the only free text today |
| scrape strategy index and success | `scraper.ts` health history, 20 entries | supports UC-2's context |
| management preset | `storage.local` | risk percent, break-even, trailing, partial TP |
| active exchange account name | `GET_ACTIVE_EXCHANGE` | label only |

### 5.3 New capture required

| Field | Where from | Needed by |
|---|---|---|
| `drawing.label` - the position tool's own text | `api.getProperties().text`, alongside the `stopLevel` / `profitLevel` reads already in `findPositionToolByChartApi` | UC-2 |
| `page.title` | `document.title`, already used by two scraper fallbacks | UC-2 context |
| `user.tags` | `GET /api/v1/journal/setup-tags`, already called by the extension | UC-1, UC-3 |
| `user.calibration` for the matched tag: `n`, `p_win`, `avg_r_win`, `avg_r_loss` | `calibration.rs` aggregates, read server-side | UC-1 confidence context |
| `notes` | `journal_entries.notes`, post-close | UC-3 |

Only the first two are genuinely new DOM reads, and both are on a field the scraper already opens.

### 5.4 State shape

One object per request, with named fields, per the docs' guidance to use an object so each part has a descriptive name and its relationships stay clear.

| Field | Type | Content |
|---|---|---|
| `setup.symbol` | string | normalized symbol |
| `setup.side` | `"LONG" \| "SHORT"` | closed set |
| `setup.entry`, `setup.stop`, `setup.target` | number | scraped or user-corrected |
| `setup.timeframe` | string | normalized |
| `setup.scraped` | bool per field | which values the machine produced |
| `drawing.label` | string, optional | the trader's own annotation on the drawing |
| `derived.rr`, `derived.stop_distance_pct` | number | **computed in code**, passed as text |
| `scrape.strategy`, `scrape.health` | int, array | which fallback ran, recent success rate |
| `page.title` | string | page identity |
| `user.tags` | string[] | the user's existing tag vocabulary |
| `user.calibration` | object, optional | shrunk stats for the candidate tag |

**Computed in code, never by Jev:** R:R, stop distance percent, tick size, risk amount, position size, margin capacity, exposure, and every number on the money path.
Jev receives text and returns labels and probabilities. Numbers stay deterministic.

**Excluded from state:** the JWT, the wallet address, exchange API credentials, raw account balances, and anything not needed to answer the questions asked.
State goes to a third party, so the default is to omit.
The extension's own listing copy promises "no data collection", and this design should not quietly break that.
If state leaves the browser, the privacy copy and `testudo.vip/privacy` need to say so first.

### 5.5 Boundary parsing

Per the constructive-modeling rules the repo already follows, the answer type is a discriminated union on `type`, not a bag with optional fields.

| Variant | Fields |
|---|---|
| `noul` | `noul` (number 0..1) |
| `choice` | `choice` (string), `probabilities` (map), `confidence` (number) |
| `score` | `score` (number), `legend` (map), `probabilities` (map), `confidence` (number) |

Add a `JudgmentResponseSchema` alongside the existing zod schemas, mirroring the answer variant.
No `isError` boolean, no optional `errorMessage`. A failed judgement is a separate result variant, not a payload with a flag on it.

### 5.6 Prompt construction rules

| Rule | Reason |
|---|---|
| The full judgement goes in `instructions`; question IDs never carry meaning | IDs are not sent to the model |
| Reference state with backticked paths | The docs recommend explicit paths to bind a judgement to specific parts of a structured state |
| Ask one narrow judgement per question; split independent dimensions | A judgement a knowledgeable person makes in a second is the target |
| Keep a `no_match` outcome in every Choice | The docs require it when nothing fits |
| Include code-only thresholds outside the model | Weights and cutoffs stay in code so they can change without rerunning inference |

---

## 6. Guardrails and Measurement

### 6.1 Hard rules

1. Jev output never enters position sizing, risk limits, or order parameters. It maps to labels, UI advisories, and confirmation steps only.
2. The only path where a judgement changes user-visible flow is UC-2's extra confirmation, and that step never mutates a field.
3. Nothing blocks a trade on a model call. Every failure mode fails open.
4. Confidence thresholds are set from measured data on real trades, not from the cookbooks' example values.
5. `testudo-proofs/` stays untouched by this. A Lean-verified sizing primitive must not acquire a probabilistic dependency.

### 6.2 What to log from day one

Every request and response pair: a state hash, the questions, the raw answers, confidence values, the model ID that answered, latency, and token usage.
Then, on trade close, the realized outcome joined to the judgement that preceded it.

This is what makes the four failure classes separable, which the TypeSafe guidance explicitly calls for: missing evidence, model error, code error, and service failure.
Without it you cannot tell whether UC-1 is improving calibration or just relabelling noise.

### 6.3 Evaluation plan

| Phase | Measure |
|---|---|
| Offline replay | Run UC-1 over the existing tagged history. How often does the resolved tag differ from the user's literal string, and does the resolved tag match the tag the user picked next time |
| UC-2 shadow | Log verdicts without showing them. Compare against trades that were actually mis-entered or immediately closed |
| UC-3 shadow | Compare note-derived tags against tags the user later assigned manually |
| Cost and latency | Per-request tokens and p50/p95 wall time on the Alt+X path |

Ship thresholds only after the shadow phase.
The docs are direct on this: typed output guarantees the interface, not truth, and performance must be validated in the target domain.

---

## 7. Gaps and Prerequisites

Blocking, in order.

| # | Gap | Work |
|---|---|---|
| 1 | TypeSafe account and key | Not present in the repo. Need a key, and a decision on which environment gets it first |
| 2 | Privacy copy | `store-assets/store-listing-copy.md` and `testudo.vip/privacy` currently promise no data collection. Sending trade context to a third party changes that, so the copy must land before the feature ships |
| 3 | Scraper must read `drawing.label` | One new field on `PositionToolData` and the existing `getProperties()` call. UC-2 cannot be evaluated without it |
| 4 | Calibration read path | Aggregates are loaded today only inside `create_trade`. UC-1's context either needs a read path or is dropped as optional |
| 5 | Judgement logging | No table for it. `trade_events` may be reused, or a dedicated table added |
| 6 | Extension not deployed | The last handoff records the extension as built-but-not-deployed while the journal was shipped. Confirm the deploy path before adding surface area |

Not blocking, worth noting: the last `HANDOFF.md` records an unresolved Hyperliquid spot-to-perp transfer bug.
That is unrelated to this proposal, but it is the open item on the branch you would be branching from.

---

## 8. Proposed Sequence

| Phase | Scope | Exit criteria |
|---|---|---|
| 0 | Agreement on this document | Use cases and routing settled |
| 1 | Server-side client plus `GET /api/v1/health/typesafe` | Key works, errors typed, fail-open proven with the key unset |
| 2 | UC-1 offline replay | Measured tag-resolution rate over existing history |
| 3 | UC-1 wired into the modal, advisory only | No change to any order parameter |
| 4 | Scraper reads the drawing label, UC-2 in shadow | Verdicts logged, not shown |
| 5 | UC-2 shown, UC-3 in shadow | No false-positive block observed in the shadow window |
| 6 | UC-3 live, coach consumes the new fields | Attribution review |

No code until phase 0 closes.

---

## 9. Decisions Required

Three, and they change the shape of the work.

**A. Use case scope.** UC-1 alone is small, low-risk, and lands on the existing calibrated sizing path.
UC-2 adds a safety gate but needs a new DOM read and carries false-positive risk.
UC-3 is the largest and reaches into the journal rather than the extension.
Recommendation: UC-1 first, because its failure mode is the cheapest and its payoff is measured against existing history.

**B. UC-2 strength.** Advisory confirmation step only, or hard block on a high-confidence mismatch.
Recommendation: advisory, with verdicts logged to `trade_events` until there is evidence that the check is right more often than the scraper is.

**C. Privacy posture.** Whether sending trade context to `api.typesafe.ai` is acceptable, and whether zero-retention terms are required before anything leaves the browser.
This gates phases 3 and beyond.

Also worth confirming: is there any Sniper build separate from the in-repo `testudo-extension`?
Research says no published extension by that name exists, and this document was built from the code in this repo.

---

## Appendix: Sources

| Source | Used for |
|---|---|
| `https://docs.typesafe.ai/llms.txt` | Documentation index |
| `https://docs.typesafe.ai/api.md` | Endpoint, request and response shapes, error codes, backoff guidance |
| `https://docs.typesafe.ai/models.md` | Model IDs, aliases, rate limits, context, pricing, data handling |
| `https://docs.typesafe.ai/concepts/state.md` | State formats and structuring guidance |
| `https://docs.typesafe.ai/primitives.md` | Primitive comparison, asking several questions together |
| `https://docs.typesafe.ai/primitives/choice.md`, `noul.md`, `score.md` | Per-primitive criteria and answer shapes |
| `https://docs.typesafe.ai/confidence.md` | Confidence semantics |
| `https://docs.typesafe.ai/cookbooks/function_calling.md` | Closed-set argument mapping, the pattern UC-1 follows |
| `https://docs.typesafe.ai/cookbooks/parallel_questions.md` | Measured batching benefit |
| `testudo-extension/src/scraper.ts`, `content.ts`, `background/*.ts`, `components/TradeForm.tsx` | Extension flow and captured data |
| `testudo-exchange/crates/router/src/services/calibration.rs`, `sizing_preview.rs`, `coach/*` | Calibration key, sizing reasoning, coach precedent for server-side model calls |
| `testudo-exchange/crates/router/src/main.rs`, `types/app.rs`, `Cargo.toml` | Secret handling, `AppState`, existing `reqwest` dependency |
| `testudo-journal/src/components/journal/*`, `testudo-exchange/crates/sqlx_postgres/migrations/*` | Note storage, tags, Dignitas journal-consistency weight |
| `testudo-extension/store-assets/store-listing-copy.md`, `testudo-extension/build.ts` | Published name, privacy claims, `testudo-sniper` add-on ID |
