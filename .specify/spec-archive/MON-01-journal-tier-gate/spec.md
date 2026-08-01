# Specification: Journal Tier Gate — Free 30-Day Window, Pro Unlimited

**Spec ID:** MON-01-journal-tier-gate
**Date:** 2026-05-01
**Status:** Draft
**Class:** Feature / Monetization
**Priority:** P0 — Unlocks the first paid surface in the platform; nothing else can be sold until this exists.
**Depends on:** None (first in MON series)
**Series:** MON-01 (tier gate) → MON-02 (payment ingress, TBD: Stripe vs USDC vs NFT pass) → MON-03 (admin tier-management UI)

---

## Problem Statement

The journal — equity curve, trade history, Dignitas score, R-multiple analytics —
is the part of Testudo that compounds in user value over time. The execution
layer (extension + sizing + routing) is acquisition: it should be free forever
because gating speed and risk discipline poisons the product's mission. The
journal is monetization: it gets stickier the longer you use it, and it is
the surface a user will pay to keep.

Right now there is no notion of a paid tier. Every authenticated user can
read every endpoint under `/api/v1/journal/**` and `/api/v1/dignitas/**`
without limit. There is no `tier` column on `users`, no enforcement at the
handler layer, and no rendering distinction in `testudo-journal/`. Until
this exists, the pricing page is theatre and there is no way to charge a
user even if they ask to pay.

This spec adds the gate **only** — not the payment system. A user is `free`
by default; promotion to `pro` happens via a manual SQL `UPDATE` for the
first cohort. That is sufficient to (a) hand-trade payment with the first
~10 reservers from the pricing page, (b) prove the gate works end-to-end,
and (c) define the contract MON-02 will hook payment into. Splitting gate
from payment lets us ship the visible monetization surface this week
instead of waiting on payment-rail decisions (Stripe vs crypto-native).

The design principle: **show locked content, do not hide it.** A free user
viewing trades older than 30 days sees ghost rows with date + symbol and a
lock icon — not a truncated list with no signal. Loss aversion is stronger
than feature aversion. The same applies to the Dignitas card: render the
card with blurred numbers and an [UNLOCK] overlay rather than a 403 the
frontend has to guess at.

---

## User Stories

- **As a free user**, I want to see the last 30 days of my trades in full
  fidelity, so that the journal is genuinely useful and I have something
  to lose by churning.
- **As a free user**, I want to see ghost rows for older trades with a
  clear unlock prompt, so that the value of upgrading is visible rather
  than invisible.
- **As a pro user**, I want unlimited journal history and full Dignitas
  analytics, so that what I paid for matches the pricing page promise.
- **As a solo operator**, I want to promote a user from free → pro with a
  single SQL `UPDATE`, so that I can hand-trade payment with the first
  cohort before MON-02 ships a payment rail.
- **As the frontend**, I want a single source of truth for the user's
  tier and entitlements on every authenticated response, so that gating
  decisions in the UI never drift from server enforcement.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `users` table has `tier text not null default 'free'` constrained to `('free','pro')` and `tier_expires_at timestamptz null`. | High | Migration |
| FR-2 | A user with `tier='pro'` AND (`tier_expires_at IS NULL OR tier_expires_at > now()`) is "effectively pro" everywhere; otherwise effectively free. Encoded once in a helper, not duplicated per handler. | High | Router (auth) |
| FR-3 | `GET /api/v1/auth/me` includes `tier`, `tier_expires_at`, and a derived `entitlements: { journal_history_days: number\|null, dignitas: bool, analytics: bool }`. `null` means unlimited. | High | Router (auth) |
| FR-4 | `GET /api/v1/journal/trades` for free users clips data rows to `closed_at >= now() - interval '30 days'` AND returns `total_unclipped`, `locked_count`, and `locked_preview: [{closed_at, symbol, side}]` (max 50 most-recent ghost rows beyond the visible window). Pro users get the existing behavior unchanged. | High | Router (journal) |
| FR-5 | All `GET /api/v1/journal/analytics/**` endpoints return `403 {reason: "pro_required", preview: <minimal teaser>}` for free users. The teaser for `/overview` includes only `trade_count` and `win_rate` from the visible 30-day window — enough to render a useful, non-blurred header on the free dashboard. | High | Router (journal) |
| FR-6 | All `GET /api/v1/dignitas/**` (score, identity, handle resolution) return `403 {reason: "pro_required"}` for free users. Public profile (`/api/v1/public/profile/{handle}`) is unaffected — public is public. | High | Router (dignitas) |
| FR-7 | `GET /api/v1/journal/trades/{id}` returns `403 {reason: "pro_required_old_trade"}` if the trade's `closed_at` is older than 30 days AND the user is free. The list endpoint will not surface IDs for old trades; this is defense-in-depth. | High | Router (journal) |
| FR-8 | Frontend renders ghost rows in the trades table for `locked_preview` entries with a lock glyph and a single "UNLOCK FULL HISTORY" CTA at the bottom of the visible list. Pro users see no ghost rows. | High | Journal (UI) |
| FR-9 | Frontend renders a blurred Dignitas card (existing layout, opacity-30 numbers, [UNLOCK] overlay) for free users. The card never disappears — the deprivation must be visible. | High | Journal (UI) |
| FR-10 | Frontend `/me` consumer caches `entitlements` and exposes `useTier()` + `useEntitlements()` hooks; no component reads `tier` from a raw response. | Medium | Journal (lib) |
| FR-11 | Manual promotion path: `UPDATE users SET tier='pro', tier_expires_at=NULL WHERE wallet_address = $1` is documented in `MANUAL_TIER_OPS.md` alongside the spec, including how to demote and how to set a time-bounded trial. | Medium | Ops |
| FR-12 | Tests: handler-level integration tests for FR-4, FR-5, FR-6, FR-7 covering both `free` and `pro` paths, including the `tier_expires_at` expiry boundary. | High | Router (tests) |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Migration + `users` reads/writes `tier`. `entitlements` shape published from `/me`. No handlers gated yet. Existing 972 tests still pass. | Schema + auth surface non-regressive. |
| CP-2 | `list_trades` clips for free users; ghost preview returned. Frontend renders ghost rows. End-to-end: free user sees 30-day window + ghost rows; pro user sees full history. | The flagship gate. Visible monetization. |
| CP-3 | Dignitas + analytics endpoints gated. Frontend renders blurred card + locked analytics tabs. Manual promotion path documented and exercised. | Full gate complete; ready to hand-trade payment. |

Each checkpoint is independently committable. CP-1 is reversible with a
single down-migration; CP-2 and CP-3 only add gating on top of CP-1's
schema, so rollback = revert handler diff.

### Migration

**File:** `testudo-exchange/crates/sqlx_postgres/migrations/20260501000000_users_tier.up.sql`

```sql
-- MON-01: Tier column for journal monetization gate.
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS tier TEXT NOT NULL DEFAULT 'free',
    ADD COLUMN IF NOT EXISTS tier_expires_at TIMESTAMPTZ NULL;

ALTER TABLE users
    ADD CONSTRAINT users_tier_check CHECK (tier IN ('free', 'pro'));

-- Partial index: lookups for active pro users (e.g. expiry sweeps later).
CREATE INDEX IF NOT EXISTS idx_users_tier_pro_active
    ON users (tier, tier_expires_at)
    WHERE tier = 'pro';
```

**Down:**

```sql
DROP INDEX IF EXISTS idx_users_tier_pro_active;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_tier_check;
ALTER TABLE users
    DROP COLUMN IF EXISTS tier_expires_at,
    DROP COLUMN IF EXISTS tier;
```

### Tier Helper (single source of truth)

**New file:** `testudo-exchange/crates/router/src/services/tier.rs`

```rust
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Free,
    Pro,
}

#[derive(Debug, Clone, Serialize)]
pub struct TierState {
    pub tier: Tier,
    pub tier_expires_at: Option<DateTime<Utc>>,
    pub entitlements: Entitlements,
}

#[derive(Debug, Clone, Serialize)]
pub struct Entitlements {
    /// `None` = unlimited. `Some(30)` = last 30 days only.
    pub journal_history_days: Option<i64>,
    pub dignitas: bool,
    pub analytics: bool,
}

pub const FREE_JOURNAL_DAYS: i64 = 30;
pub const FREE_LOCKED_PREVIEW_LIMIT: i64 = 50;

impl TierState {
    pub fn from_row(tier: &str, expires: Option<DateTime<Utc>>) -> Self {
        let effective = match tier {
            "pro" if expires.map_or(true, |e| e > Utc::now()) => Tier::Pro,
            _ => Tier::Free,
        };
        let entitlements = match effective {
            Tier::Pro => Entitlements {
                journal_history_days: None,
                dignitas: true,
                analytics: true,
            },
            Tier::Free => Entitlements {
                journal_history_days: Some(FREE_JOURNAL_DAYS),
                dignitas: false,
                analytics: false,
            },
        };
        Self { tier: effective, tier_expires_at: expires, entitlements }
    }

    pub async fn for_user(pool: &PgPool, user_id: Uuid) -> sqlx::Result<Self> {
        let row: (String, Option<DateTime<Utc>>) = sqlx::query_as(
            "SELECT tier, tier_expires_at FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(Self::from_row(&row.0, row.1))
    }

    pub fn is_pro(&self) -> bool { matches!(self.tier, Tier::Pro) }
}
```

Every handler that needs to gate calls `TierState::for_user(&pool, user.user_id).await?`
once at the top, then branches on `state.is_pro()`. No tier logic
is open-coded in handlers.

### `/me` Response Extension

**File:** `testudo-exchange/crates/router/src/routes/auth.rs` (or wherever `/me` lives — locate via `grep -rn "fn me\|/me" routes/auth*.rs`)

```rust
// Existing /me response, extended:
#[derive(Serialize)]
struct MeResponse {
    user_id: Uuid,
    wallet_address: String,
    // ... existing fields
    tier: Tier,
    tier_expires_at: Option<DateTime<Utc>>,
    entitlements: Entitlements,
}
```

Backwards-compatible: new fields, no removals. The journal frontend
will read these; older extension builds ignore unknown JSON keys.

### `list_trades` Clip + Ghost Preview

**File:** `testudo-exchange/crates/router/src/routes/journal.rs`

Modify the existing handler (lines 278–442). At the top, after parsing
query params:

```rust
let tier_state = TierState::for_user(pool, user.user_id).await
    .map_err(|e| {
        tracing::error!("Failed to load tier: {e}");
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

// Free users: force-add a closed_at >= cutoff condition.
let free_cutoff: Option<DateTime<Utc>> = match tier_state.entitlements.journal_history_days {
    Some(days) => Some(Utc::now() - chrono::Duration::days(days)),
    None => None,
};
if let Some(cutoff) = free_cutoff {
    conditions.push(format!("jt.closed_at >= ${bind_idx}::timestamptz"));
    str_binds.push(cutoff.to_rfc3339());
    bind_idx += 1;
}
```

Then after the count query, when free, run a second cheap query to get
the locked preview + locked total:

```rust
#[derive(Serialize, sqlx::FromRow)]
struct LockedPreview {
    closed_at: DateTime<Utc>,
    symbol: String,
    side: String,
}

let (locked_count, locked_preview): (i64, Vec<LockedPreview>) = if let Some(cutoff) = free_cutoff {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM journal_trades WHERE user_id = $1 AND closed_at < $2"
    )
    .bind(user.user_id).bind(cutoff)
    .fetch_one(pool).await.unwrap_or(0);

    let preview = sqlx::query_as::<_, LockedPreview>(
        "SELECT closed_at, symbol, side FROM journal_trades \
         WHERE user_id = $1 AND closed_at < $2 \
         ORDER BY closed_at DESC LIMIT $3"
    )
    .bind(user.user_id).bind(cutoff).bind(FREE_LOCKED_PREVIEW_LIMIT)
    .fetch_all(pool).await.unwrap_or_default();

    (count, preview)
} else {
    (0, vec![])
};
```

Response shape — extend `PaginatedTrades`:

```rust
#[derive(Serialize)]
pub struct PaginatedTrades {
    pub trades: Vec<TradeApiResponse>,
    pub total: i64,                // visible total (clipped for free)
    pub page: i32,
    pub limit: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_count: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub locked_preview: Vec<LockedPreview>,
}
```

For pro users `locked_count` is `None` and `locked_preview` is empty —
unchanged on the wire from today.

### Analytics + Dignitas Gates

For each `journal::*analytics*` and `dignitas::*` handler, add at the top:

```rust
let tier_state = TierState::for_user(&app_state.pool, user.user_id).await?;
if !tier_state.is_pro() {
    return Ok(HttpResponse::Forbidden().json(serde_json::json!({
        "reason": "pro_required",
        "upgrade_url": "/pricing"
    })));
}
```

Exception: `journal::overview` returns a teaser instead of a 403 — see FR-5.
Compute `trade_count` and `win_rate` from the **clipped** dataset
(last 30 days) and return:

```json
{
  "preview": true,
  "reason": "pro_required",
  "teaser": { "trade_count": 12, "win_rate": 0.58 },
  "upgrade_url": "/pricing"
}
```

### Frontend — `useTier` + `useEntitlements`

**Files:**
- `testudo-journal/src/context/AuthContext.tsx` — extend `User` type with
  `tier`, `tier_expires_at`, `entitlements`. Already in `/me` response.
- `testudo-journal/src/lib/tier.ts` (new) — exports `useTier()`,
  `useEntitlements()`, `useIsPro()` Solid signals derived from auth context.
  Single read site for all gating.

```ts
// testudo-journal/src/lib/tier.ts
import { useAuth } from '../context/AuthContext'

export function useTier() {
  const { user } = useAuth()
  return () => user()?.tier ?? 'free'
}

export function useIsPro() {
  const tier = useTier()
  return () => tier() === 'pro'
}

export function useEntitlements() {
  const { user } = useAuth()
  return () => user()?.entitlements ?? {
    journal_history_days: 30,
    dignitas: false,
    analytics: false,
  }
}
```

### Frontend — Ghost Rows + Blurred Dignitas Card

**Trade list component** (locate via `grep -rn "trades.map\|TradeRow" testudo-journal/src/components/`): after rendering visible trades, if `locked_preview.length > 0`, render a divider row:

```
─────  LOCKED — UPGRADE TO VIEW (-N more trades) ─────
```

…then render each `locked_preview` entry as a row with `closed_at`,
`symbol`, `side`, and a lock icon in place of the P&L column.
Final row is a single button: `[ UNLOCK FULL HISTORY → /pricing ]`.

**Dignitas card**: existing component. When `!useEntitlements().dignitas`,
wrap children in `<div class="opacity-30 blur-sm pointer-events-none">`
and overlay `<div class="absolute inset-0 flex items-center justify-center">[ UNLOCK DIGNITAS ]</div>`.
The card layout itself does not change — the user sees the *shape* of what
they are missing.

### Manual Promotion Path

**File:** `.specify/specs/MON-01-journal-tier-gate/MANUAL_TIER_OPS.md`

```markdown
# Manual Tier Operations (pre-MON-02)

## Promote to PRO (lifetime)
UPDATE users SET tier='pro', tier_expires_at=NULL WHERE wallet_address = $1;

## Promote to PRO (30-day trial)
UPDATE users SET tier='pro', tier_expires_at=NOW() + INTERVAL '30 days' WHERE wallet_address = $1;

## Demote
UPDATE users SET tier='free', tier_expires_at=NULL WHERE wallet_address = $1;

## Audit: who is pro right now?
SELECT id, wallet_address, tier, tier_expires_at FROM users
WHERE tier='pro' AND (tier_expires_at IS NULL OR tier_expires_at > NOW())
ORDER BY tier_expires_at NULLS FIRST;
```

Wallet addresses come from the pricing-page reservation flow (any
authenticated SIWE session that hits `?reserve=pro` is a candidate;
log this signal in MON-02 — out of scope here).

### Paved Roads

- **`AuthenticatedUser` extractor** (`router/src/middleware/auth.rs:265`)
  already provides `user_id` and `wallet_address`. We do not extend it —
  tier is loaded per-handler so a middleware change cannot cause a
  silent gate-bypass on a handler that forgot to call the helper.
- **Existing migration pattern** — single `up.sql` + `down.sql`,
  filename `YYYYMMDDHHMMSS_<slug>.{up,down}.sql`. Verified via
  `crates/sqlx_postgres/migrations/`.
- **Existing `PaginatedTrades` response shape** — extended
  additively, no breaking changes for existing extension consumers.
- **Frontend SWR cache** (`testudo-journal/src/lib/cache.ts`) —
  `useCachedResource` will pick up new `entitlements` fields
  transparently; no cache invalidation strategy change needed.

### Files

**New:**
- `testudo-exchange/crates/sqlx_postgres/migrations/20260501000000_users_tier.up.sql`
- `testudo-exchange/crates/sqlx_postgres/migrations/20260501000000_users_tier.down.sql`
- `testudo-exchange/crates/router/src/services/tier.rs`
- `testudo-journal/src/lib/tier.ts`
- `.specify/specs/MON-01-journal-tier-gate/MANUAL_TIER_OPS.md`

**Modified:**
- `testudo-exchange/crates/router/src/routes/auth.rs` — `/me` response includes `tier` + `entitlements`
- `testudo-exchange/crates/router/src/routes/journal.rs` — clip free users in `list_trades`, gate analytics + `get_trade`
- `testudo-exchange/crates/router/src/routes/dignitas.rs` — gate all routes except public profile
- `testudo-exchange/crates/router/src/services/mod.rs` — export `tier` module
- `testudo-journal/src/context/AuthContext.tsx` — User type adds tier fields
- `testudo-journal/src/components/<TradesTable>.tsx` — ghost rows
- `testudo-journal/src/components/<DignitasCard>.tsx` — blur + overlay

### Dependencies Added

None. All implementation uses crates already in the workspace
(`sqlx`, `chrono`, `serde`, `actix-web`, `uuid`).

---

## Acceptance Criteria

- [ ] FR-1: Migration applies cleanly on a fresh DB and on a copy of prod;
      `tier='free'` for all existing rows; constraint rejects `tier='premium'`.
- [ ] FR-2: `TierState::from_row("pro", Some(yesterday))` returns Free;
      `TierState::from_row("pro", Some(tomorrow))` returns Pro;
      `TierState::from_row("pro", None)` returns Pro;
      `TierState::from_row("free", None)` returns Free.
      Covered by unit test in `services/tier.rs`.
- [ ] FR-3: `GET /me` for a free user returns `entitlements.journal_history_days = 30`,
      `dignitas = false`, `analytics = false`. Pro user returns `null` / `true` / `true`.
- [ ] FR-4: Free user with 100 closed trades spanning 90 days sees only the
      most-recent 30 days in `trades`, with `locked_count = 70` and
      `locked_preview.length` ≤ 50, ordered DESC by `closed_at`. Pro user
      sees all 100 with `locked_count` absent.
- [ ] FR-5: `GET /api/v1/journal/analytics/equity-curve` returns 403
      `{reason: "pro_required"}` for free users.
      `GET /api/v1/journal/analytics/overview` returns the teaser shape for free users.
- [ ] FR-6: `GET /api/v1/dignitas/score` returns 403 for free users;
      `GET /api/v1/public/profile/{handle}` returns 200 (unchanged).
- [ ] FR-7: `GET /api/v1/journal/trades/<old_trade_id>` as a free user returns
      403 `{reason: "pro_required_old_trade"}` even when the ID is known.
- [ ] FR-8: Visual check — free user dashboard shows 30 days of full rows
      followed by a divider and ghost rows with lock glyph; pro user shows
      no ghosts.
- [ ] FR-9: Visual check — free user sees blurred Dignitas card with overlay;
      pro user sees the live card.
- [ ] FR-10: `useIsPro()` and `useEntitlements()` are the only places
      `tier` is read in the journal codebase. `grep -rn "\.tier" testudo-journal/src/components/`
      returns zero hits.
- [ ] FR-11: `MANUAL_TIER_OPS.md` exists; SQL snippets verified against the
      live schema.
- [ ] FR-12: Handler integration tests for free + pro paths added under
      `crates/router/tests/` and pass.
- [ ] Verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test` — all green, no new warnings.
- [ ] Verification: `cd testudo-journal && bun run build && bun run test` — all green.
- [ ] No regressions in existing 972 Rust tests.

---

## Risks

1. **Silent gate-bypass on a forgotten handler.** A new `/journal/*`
   endpoint added later without calling `TierState::for_user` would leak
   pro features to free users. **Mitigation:** add a clippy-style
   integration test that hits every registered `/journal/*` and
   `/dignitas/*` route with a free-tier session and asserts non-200 (or
   teaser-shape) for everything except `list_trades` (clipped) and
   `overview` (teaser). Catches regressions at PR time.
2. **Clip cutoff drift between `total` and `locked_count`.** The two
   queries use `now()` separately and could disagree across a
   day-boundary in extreme cases. **Mitigation:** snapshot
   `let cutoff = Utc::now() - …;` once at handler entry and bind the
   same value to both queries.
3. **Existing users locked out of their own history at gate-flip.**
   Anyone who has been using the journal for more than 30 days will
   instantly find most of their data gated. **Mitigation:** at deploy
   time, run a one-shot SQL to grant a 30-day pro trial to every
   existing user (`tier='pro', tier_expires_at = now() + interval '30 days'`).
   This buys a month for the in-app upgrade prompt to work and avoids a
   "Testudo deleted my history" support fire. Document this one-shot
   in `MANUAL_TIER_OPS.md` and run it as part of the deploy checklist.
4. **Frontend consumes `entitlements` before the field rolls out
   server-side.** The journal app deploys independently of the router.
   **Mitigation:** ship router CP-1 first; verify `/me` returns the new
   fields in prod; then ship journal changes. Frontend defaults are
   safe (free) if the field is missing, so worst case is a free user
   seeing the free experience for one deploy cycle.
5. **The `ghost_preview` query is unbounded scan on `journal_trades`
   for users with thousands of historical trades.** **Mitigation:**
   `LIMIT 50` cap + the existing `(user_id, closed_at)` index pattern
   (verify via `EXPLAIN` on a large user). Add a composite index if
   the `EXPLAIN` shows a sequential scan.
6. **Pro user with expired `tier_expires_at` keeps a long-lived JWT
   showing pro entitlements.** Tier is read on every request via
   `TierState::for_user`, **not** baked into the JWT — so this risk is
   already mitigated by design. Document the choice so a future
   "let's cache tier in the JWT" optimization doesn't accidentally
   create a stale-entitlement bug.

---

## Completion Signal

This spec is complete when:
1. Migration applied to prod; all existing users granted a 30-day pro trial.
2. `/me` returns `tier` + `entitlements`; `list_trades` clips for free
   and returns ghost preview; analytics + dignitas endpoints return 403
   (or teaser) for free; trade detail returns 403 for old trades.
3. Frontend renders ghost rows and blurred dignitas card; no
   component reads `tier` outside of `useTier`/`useIsPro`/`useEntitlements`.
4. `MANUAL_TIER_OPS.md` exists and is exercised at least once
   (promote one tester wallet to pro via SQL, verify they see full
   history and dignitas).
5. `cargo clippy --all-targets && cargo test` and
   `bun run build && bun run test` (in `testudo-journal/`) green.
6. Code committed to master across `testudo-exchange/` and `testudo-journal/`.
7. The `/pricing` page's "RESERVE WITH WALLET" CTA results in a
   recordable signal (wallet address landed on the SIWE log) that the
   operator can use to promote that user via the manual SQL path.
   (MON-02 will replace the manual step with a payment flow.)
