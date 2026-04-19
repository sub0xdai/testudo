# Specification: AI Trade Coach — Weekly Behavioral Insights (MVP)

**Spec ID:** RSK-03-ai-trade-coach
**Date:** 2026-04-17
**Status:** Draft
**Class:** Feature / Backend + Frontend + Integration
**Priority:** P1 — retention moat; unlocks "AI Coach" positioning that differentiates Testudo from passive journals (Edgewonk / Tradervue / TradeZella). Ships after RSK-01 (unified risk hub) and RSK-02 (setup tagging).
**Depends on:** RSK-01 (unified risk hub — provides coach banner slot + snapshot aggregation), RSK-02 (setup tagging — provides per-setup baseline data)
**Series:** RSK-01 through RSK-04 (RSK-04 = real-time coach fast-follow, out of scope here)

---

## Problem Statement

Every existing trade journal (Edgewonk, Tradervue, TradeZella, most Notion templates) is a **passive database**: it records what a trader did but never tells them *why* they're bleeding capital. The deepest leak in retail trading is **behavioral** — revenge trading after a loss, sizing drift, session overtrading, correlation stacking, setup fatigue — and none of these show up cleanly in win-rate or profit-factor dashboards. Traders know the patterns conceptually but can't detect themselves in real time.

Testudo already captures the data to see these patterns (trade pipeline, setup tags from RSK-02, unified risk snapshot from RSK-01) but currently surfaces only aggregate statistics. The journal answers "what happened?"; it doesn't answer "what pattern did you just repeat?" That gap is the single most novel positioning space in the retail trading tooling landscape — a coach that converts abstract trading psychology into quantifiable, cited, personalized feedback.

This spec ships the **weekly MVP** of the AI Trade Coach: a weekly report combining deterministic pattern detection (Rust rules) with LLM-narrated analysis, grounded in specific trade citations. The report is delivered **entirely in-app** — as a persistent top-insight banner on the Account page (filling the coach slot reserved by RSK-01) and a full-read view at `/desk/coach` with an archive of past reports. **No email pipeline is used**, which removes infra dependencies and keeps the coach's surface area inside the product.

Real-time coaching (fires on fill events) is deliberately deferred to RSK-04 as a fast-follow; weekly-first proves detection quality, builds an eval corpus, and avoids the highest-risk context (coaching a trader mid-session) until the system's judgment is trusted.

---

## User Stories

- **As a Testudo user with ≥ 30 trades of history**, I want a weekly coach report analyzing behavioral patterns in my trading, so I start the next week with awareness of my own leaks.
- **As a user opening the Account page**, I want to see the coach's most important insight from the latest report as an always-visible banner, so live risk state and behavioral state share one screen.
- **As a user who wants the full analysis**, I want a `/desk/coach` page showing the full current report plus an archive of past weekly reports, so I can track my own pattern evolution over time.
- **As a data-grounded skeptic**, I want every narrative claim in the report to cite specific trade IDs so I can verify the coach didn't hallucinate.
- **As a new user with <30 trades**, I want a clear "unlocks after 30 trades" placeholder rather than a hollow generic report, so the coach maintains credibility from day one.
- **As a user who took a week off**, I want *no new report generated* on weeks I didn't trade, so the coach doesn't feel like a form letter.
- **As a privacy-conscious user**, I want disclosure of which LLM provider analyzes my data and the ability to disable the coach entirely, so I'm in control of what leaves the server.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Deterministic pattern detectors implemented in Rust: sizing drift after losses, frequency spike, session anomaly, setup fatigue, correlation stacking, streak risk (wins & losses) | High | db-processor |
| FR-2 | `CoachDigest` JSON composer: computes user baseline (30-day rolling) + flags this-week candidate patterns with trade-ID evidence + includes only flagged raw trades | High | db-processor |
| FR-3 | `NarratedReport` generator: calls OpenAI-compatible LLM endpoint with cached prompt prefix + `CoachDigest`, receives structured narrative with mandatory trade-ID citations | High | db-processor |
| FR-4 | Citation validator: every narrative claim must reference a trade ID present in the digest; reports failing validation are rejected and fall back to stats-only | High | db-processor |
| FR-5 | Weekly cron runs Sunday 18:00 UTC; early-exits for users with <30 total trades OR <3 trades in the past 7 days with `skip_reason` logged and **no report generated** | High | db-processor |
| FR-6 | `/desk/coach` route renders current week's full report (layered: deterministic stats block on top, LLM narrative with trade-citation links below) + archive of past reports, paginated | High | journal/frontend |
| FR-7 | Account page `CoachBanner` slot (reserved in RSK-01) displays the coach's top insight from the latest report with a dismiss action persisted per-user-per-week; clicking routes to `/desk/coach` | High | journal/frontend |
| FR-8 | Provider abstraction via OpenAI-compatible client; `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL` env-configurable; default to DeepSeek-V3 with prompt caching enabled | High | db-processor |
| FR-9 | User preference to disable coach entirely (opt-out); default is opt-in | Medium | router + journal |
| FR-10 | Privacy disclosure page / help entry documenting which LLM provider is used and what data is sent | Medium | journal/frontend |
| FR-11 | Prompt structure ensures cache hit on stable prefix (system role + pattern taxonomy + output schema + few-shot examples); per-user payload is the only non-cached segment | High | db-processor |
| FR-12 | LLM failure (timeout, rate limit, invalid response) does not block report storage; stats-only report is persisted and the `/desk/coach` view shows `● coach unavailable this week` in the narrative slot | High | db-processor |
| FR-13 | New report available detection: when a report is stored for the current week and the user's latest-viewed-report timestamp is older, the `CoachBanner` surfaces a subtle `● new` indicator until the user visits `/desk/coach` | Medium | journal/frontend |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Pattern detectors (6 rules) in `crates/db-processor/src/coach/patterns/`, unit-tested with fixture weeks that each trigger exactly one pattern | Detection correctness in isolation |
| CP-2 | `CoachDigest` composer aggregates baselines + flagged candidates into compact JSON; golden-snapshot tests against fixture weeks | Shape and content of LLM input are stable and testable |
| CP-3 | Narrator adapter (OpenAI-compatible via `async-openai`), mocked in tests; citation validator enforces grounding | LLM integration works, hallucinated citations are caught |
| CP-4 | `/desk/coach` route renders a stats-only digest (no LLM yet); archive list wired to PG storage | Frontend shape is correct before narrative lands |
| CP-5 | LLM narration enabled end-to-end; stats-only fallback path tested by injecting narrator failures | Graceful degradation verified |
| CP-6 | Weekly cron scheduled (tokio-cron or pg_cron); runs on Sunday 18:00 UTC; persists reports to `coach_reports` table; skip-empty-weeks rule enforced | Scheduled generation works without any notification infra |
| CP-7 | `CoachBanner` on Account page reads latest report, surfaces top insight with `● new` indicator; dismiss action persists per week | RSK-01 slot fulfilled; in-app discovery works |
| CP-8 | Opt-out preference + privacy disclosure page shipped | FR-9, FR-10 |

Each checkpoint is independently shippable. The spec achieves meaningful user value at CP-4 (stats-only weekly report in-app), with CP-5 through CP-8 layering the "AI" narrative and UX polish on top.

### Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│  testudo-exchange/crates/db-processor/src/coach/                 │
│                                                                  │
│   patterns/                 ← deterministic detectors (Rust)    │
│     sizing_drift.rs          (stateless, unit-testable)         │
│     frequency_spike.rs                                          │
│     session_anomaly.rs                                          │
│     setup_fatigue.rs         ← reads RSK-02 setup tags          │
│     correlation_stack.rs     ← reuses RSK-01 bucket logic       │
│     streak_risk.rs                                              │
│                                                                  │
│   digest.rs                 ← compose baseline + flagged         │
│     fn build_digest(user_id, week) -> CoachDigest                │
│                                                                  │
│   narrator.rs               ← OpenAI-compatible LLM call         │
│     fn narrate(digest: CoachDigest) -> Result<NarratedReport>    │
│     uses prompt-cached prefix                                    │
│                                                                  │
│   validator.rs              ← citation grounding check           │
│     fn validate(report, digest) -> Result<NarratedReport>        │
│                                                                  │
│   schedule.rs               ← weekly cron, skip rules            │
│                                                                  │
│   types.rs                  ← CoachDigest, NarratedReport, ...   │
└──────────────────────────────────────────────────────────────────┘
              │
              ▼
      coach_reports table (PG) — archive of all generated reports
              │
              ▼
┌──────────────────────────────────────────────────────────────────┐
│  testudo-journal                                                 │
│   pages/Coach.tsx               ← /desk/coach route              │
│   components/coach/                                              │
│     CoachReport.tsx             ← full layered report render     │
│     CoachArchive.tsx            ← past reports list              │
│     CoachBanner.tsx (from RSK-01 placeholder — now implemented)  │
└──────────────────────────────────────────────────────────────────┘
```

No external notification infra (no email, no push, no SMS). Reports land in the database on a schedule; the user discovers them via the in-app banner the next time they open Testudo.

### Key Types

```rust
// testudo-exchange/crates/db-processor/src/coach/types.rs

#[derive(Serialize, Deserialize, Debug)]
pub struct CoachDigest {
    pub user_id: Uuid,
    pub week_start: DateTime<Utc>,
    pub week_end: DateTime<Utc>,
    pub baseline: UserBaseline,
    pub week_stats: WeekStats,
    pub flagged_patterns: Vec<FlaggedPattern>,
    pub flagged_trades: Vec<TradeEvidence>, // only trades referenced by flags
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserBaseline {
    // 30-day rolling, computed from journal
    pub avg_trades_per_day: Decimal,
    pub avg_position_size_usd: Decimal,
    pub typical_session_hours_utc: Vec<u8>, // e.g. [13,14,15,16] = user's active window
    pub win_rate: Decimal,
    pub avg_r_multiple: Decimal,
    pub setup_baselines: HashMap<String, SetupBaseline>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FlaggedPattern {
    pub pattern: PatternKind,          // enum: SizingDrift | FrequencySpike | ...
    pub severity: Severity,            // Info | Notable | Concerning
    pub evidence: Vec<Uuid>,           // trade IDs, all must appear in flagged_trades
    pub metrics: serde_json::Value,    // pattern-specific numbers (e.g. size_multiplier: 2.1)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NarratedReport {
    pub digest_id: Uuid,
    pub headline: String,              // the "top insight" shown in CoachBanner
    pub sections: Vec<NarrativeSection>,
    pub model_used: String,
    pub generated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NarrativeSection {
    pub pattern: PatternKind,
    pub body: String,                  // markdown-ish, with [T-abc123] citation tokens
    pub citations: Vec<Uuid>,          // must be subset of flagged_trades ids
}
```

### Prompt Structure (cache-optimized)

```
┌─────────────────────────────────────────────────────┐
│ CACHED PREFIX (~8k tokens, stable across all calls) │
│                                                     │
│ - System role: "You are the Testudo Coach..."       │
│ - Pattern taxonomy (definitions of all 6 patterns)  │
│ - Output schema (strict JSON)                       │
│ - Citation rules: every claim MUST [T-xxx] cite     │
│ - 2-3 few-shot examples (good reports)              │
│ - Tone: direct, non-judgmental, data-first          │
└─────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────┐
│ USER PAYLOAD (~1-2k tokens, per user per week)      │
│                                                     │
│ - CoachDigest JSON serialized                       │
│ - "Generate a NarratedReport for this digest."      │
└─────────────────────────────────────────────────────┘
```

Target cache hit: 80-90% of total input tokens. On DeepSeek-V3 (~$0.014/M cached, $0.14/M input, $0.28/M output): **~$0.0003/report**. 1000 users × weekly = **~$0.30/week total LLM spend**.

### Pattern Detectors — MVP List

| Pattern | Signal | Baseline comparison |
|---------|--------|---------------------|
| **SizingDrift** | Avg size of last 3 post-loss trades > 1.5× 30-day baseline size | 30-day median size, per-setup if RSK-02 tag present |
| **FrequencySpike** | Trades in any 6h window > 90th percentile of rolling 30-day windows | 30-day p90 trades-per-6h |
| **SessionAnomaly** | ≥ 2 trades this week outside the user's typical active hours (defined by top 4 hours of last 30d) | 30-day activity histogram |
| **SetupFatigue** | R-multiple for a tagged setup (RSK-02) trailing 10 trades < 0.5× all-time R-multiple for that setup | Per-setup all-time avg R |
| **CorrelationStack** | ≥ 3 positions in same asset family, same direction, held concurrently for > 4h | Reuses RSK-01 asset-family bucket logic |
| **StreakRisk** | 3+ consecutive losses OR 5+ consecutive wins with size increasing | Consecutive-loss / consecutive-win counters |

All thresholds are configurable via `coach_config` table so they can be tuned without a redeploy.

### Paved Roads

- **`crates/db-processor`** is the natural home for scheduled background work (it already hosts trade ingestion and stats computation per memory).
- **pg_queue / tokio-cron** — existing patterns for scheduled and async work.
- **RSK-01's `/api/risk/snapshot`** aggregation logic — reused for `CorrelationStack` pattern.
- **RSK-02's setup tags** — reused for `SetupFatigue` pattern.
- **`rust_decimal::Decimal`** — all pattern math (no `f64`).
- **`async-openai` crate** — OpenAI-compatible client, points at any provider via `OPENAI_BASE_URL`.
- **`HELP` tooltip system** — help text for each pattern section.
- **`PageSubHeader`, signal colors, `font-mono`** — existing aesthetic tokens for `/desk/coach`.

### Files

**New (backend):**
- `testudo-exchange/crates/db-processor/src/coach/mod.rs`
- `testudo-exchange/crates/db-processor/src/coach/types.rs`
- `testudo-exchange/crates/db-processor/src/coach/patterns/mod.rs`
- `testudo-exchange/crates/db-processor/src/coach/patterns/sizing_drift.rs`
- `testudo-exchange/crates/db-processor/src/coach/patterns/frequency_spike.rs`
- `testudo-exchange/crates/db-processor/src/coach/patterns/session_anomaly.rs`
- `testudo-exchange/crates/db-processor/src/coach/patterns/setup_fatigue.rs`
- `testudo-exchange/crates/db-processor/src/coach/patterns/correlation_stack.rs`
- `testudo-exchange/crates/db-processor/src/coach/patterns/streak_risk.rs`
- `testudo-exchange/crates/db-processor/src/coach/digest.rs`
- `testudo-exchange/crates/db-processor/src/coach/narrator.rs`
- `testudo-exchange/crates/db-processor/src/coach/validator.rs`
- `testudo-exchange/crates/db-processor/src/coach/schedule.rs`
- `testudo-exchange/crates/db-processor/tests/coach_patterns_test.rs`
- `testudo-exchange/crates/db-processor/tests/coach_digest_snapshot_test.rs`
- `testudo-exchange/crates/db-processor/tests/coach_validator_test.rs`
- `testudo-exchange/crates/sqlx_postgres/migrations/NNNN_coach_reports.sql`
- `testudo-exchange/crates/router/src/routes/coach.rs` — `GET /api/coach/latest`, `GET /api/coach/archive`, `PATCH /api/coach/preference`, `PATCH /api/coach/dismiss-banner`

**New (frontend):**
- `testudo-journal/src/pages/Coach.tsx`
- `testudo-journal/src/components/coach/CoachReport.tsx`
- `testudo-journal/src/components/coach/CoachArchive.tsx`
- `testudo-journal/src/components/coach/NarrativeBlock.tsx` (renders `[T-xxx]` citation tokens as trade links)

**Modified:**
- `testudo-journal/src/components/account/CoachBanner.tsx` — RSK-01 placeholder now fetches and renders live top insight with `● new` indicator
- `testudo-journal/src/index.tsx` — register `/coach` route
- `testudo-journal/src/components/Layout.tsx` — add Coach to top nav (or nest under Account — TBD at ship)
- `testudo-journal/src/api/client.ts` — add `fetchLatestCoachReport`, `fetchCoachArchive`, `setCoachPreference`, `dismissCoachBanner`
- `testudo-journal/src/lib/help-content.ts` — help entries for each pattern type
- `testudo-exchange/crates/db-processor/Cargo.toml` — add `async-openai`, `chrono`
- `testudo-exchange/crates/router/src/routes/mod.rs` — wire coach routes

### Dependencies Added

- `async-openai = "0.27"` (or current) — OpenAI-compatible LLM client for DeepSeek/GLM/any compatible provider

### Env Configuration

```
OPENAI_BASE_URL=https://api.deepseek.com/v1    # or GLM, or OpenRouter
OPENAI_API_KEY=<key>
OPENAI_MODEL=deepseek-chat                      # swappable: glm-4-flash, claude-sonnet-4-6, ...
COACH_ENABLED=true                              # global kill-switch
COACH_CRON=0 18 * * 0                           # Sunday 18:00 UTC
COACH_MIN_LIFETIME_TRADES=30
COACH_MIN_WEEK_TRADES=3
```

---

## Acceptance Criteria

- [ ] All 6 pattern detectors implemented, each with unit tests using fixture weeks that trigger the pattern cleanly and non-triggering fixtures
- [ ] `CoachDigest` composer produces deterministic output (snapshot-tested)
- [ ] Citation validator rejects any narrative claim referencing a trade ID not in the digest
- [ ] Weekly cron runs on schedule, skips users with <30 lifetime trades or <3 trades this week, logs `skip_reason` and persists **no report**
- [ ] Stats-only fallback persists a valid report when LLM call fails (simulated by pointing at a 404 URL in a test); `/desk/coach` renders it with `● coach unavailable this week` in the narrative slot
- [ ] `/desk/coach` route renders latest report + paginated archive
- [ ] `CoachBanner` on Account shows top insight with `● new` indicator when a new report has been generated since the user last visited `/desk/coach`; dismiss action persisted per-week
- [ ] Visiting `/desk/coach` clears the `● new` indicator on the banner
- [ ] User opt-out preference respected (no job runs, no report persisted, `/desk/coach` shows opt-out message, banner is hidden)
- [ ] Privacy disclosure page live; documents LLM provider and data surface
- [ ] Prompt cache hit rate verified ≥ 70% on consecutive runs (log cache metadata from provider response)
- [ ] Backend verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] Frontend verification: `cd testudo-journal && bun run build`
- [ ] End-to-end manual: seed a test user with a fixture week that deliberately triggers 3+ patterns, run the coach manually, inspect `/desk/coach` + banner

---

## Risks

1. **LLM fabricates patterns or trade IDs not in the digest.** *Mitigation:* citation validator is a hard gate — invalid reports are rejected and the user receives stats-only. Log every rejection to tune the prompt.
2. **Cold-start disappointment.** Users who install Testudo expect magic day-one; if the 30-trade threshold isn't clearly surfaced, they'll churn. *Mitigation:* `/desk/coach` pre-threshold shows an explicit progress bar ("14/30 trades to unlock the coach"). Landing page copy mirrors this expectation.
3. **Generic or hollow narration.** If rules flag nothing, the narrative is vacuous. *Mitigation:* skip-empty-week rule (no report persisted, banner continues to show last meaningful report until next meaningful week); `/desk/coach` shows "quiet week — nothing notable" rather than forcing output.
4. **No push surface means users miss reports.** Without email or push notifications, a new weekly report can sit unread until the user next opens Testudo. *Mitigation:* `CoachBanner` on Account is the always-visible discovery surface — any user checking their risk sees the new insight immediately. Accept the trade-off: no external infra, lower engagement, higher signal-to-noise (no inbox spam).
5. **Provider outage / rate limit.** DeepSeek and GLM have had availability issues. *Mitigation:* FR-12 fallback (stats-only) is automatic; OpenAI-compatible abstraction means swapping providers is a config change, not a rewrite. OpenRouter as emergency fallback.
6. **Cost explosion from bad actors.** A malicious user scripting trade creation could trigger large digests. *Mitigation:* digest size is bounded by flagged-only trades + baseline aggregates (not raw history); per-user rate limit on manual coach trigger if we add one; overall weekly job is opt-in gated.
7. **Privacy perception.** Some users will object to PRC-hosted inference over their trading data. *Mitigation:* FR-9 opt-out + FR-10 disclosure; Phase 2 premium tier with self-hosted local model planned as RSK-04.
8. **Pattern false positives create coach fatigue.** If the coach flags SizingDrift every week on someone whose natural style is variable sizing, it becomes noise. *Mitigation:* `Severity::Info | Notable | Concerning` triaging; only Notable+ appear in the banner; Info appears only in the full `/desk/coach` view. Thresholds in `coach_config` table for tuning.
9. **Tone risk — sounds like a scolding parent.** *Mitigation:* system prompt explicitly bans moralizing language; few-shot examples demonstrate direct, data-first, non-judgmental tone. Manual review of first 20 generated reports before production rollout.

---

## Completion Signal

This spec is complete when:
1. All FR-1 through FR-13 implemented and tested
2. All acceptance criteria checked off
3. Manual QA: 3 test users with distinct behavioral profiles have accurate, correctly-cited weekly reports generated over two consecutive real weeks, visible in the banner and `/desk/coach`
4. First 20 production reports reviewed by spec owner before the full rollout is enabled via `COACH_ENABLED=true`
5. Prompt cache hit rate verified in production logs
6. Backend `cargo clippy --all-targets && cargo test` and frontend `bun run build` both pass
7. Code committed to master via conventional commit: `feat(rsk-03): weekly AI trade coach — pattern detection + narrated report (in-app only)`
8. `/desk/coach` help copy clearly sets expectation: "Unlocks after 30 trades. Weekly report. Your data is analyzed by [provider] — you can opt out."
9. RSK-04 (real-time coach) scoped as fast-follow with eval criteria informed by RSK-03 report corpus
