# Specification: Dignitas Public Profile — Handles + Shareable Identity

**Spec ID:** ENG-01b-dignitas-public-profile
**Date:** 2026-04-20
**Status:** Draft
**Class:** Feature / Backend + Frontend
**Priority:** P2 — converts a private score into portable credential; ships after ENG-01a
**Depends on:** ENG-01a (reads `dignitas_history` for score display)
**Parent series:** ENG-01 (carved 2026-04-20 from monolithic ENG-01-dignitas-living-score)
**Siblings:** ENG-01a (score-living baseline), ENG-01c (streak counter)

---

## Problem Statement

Once a user has a living Dignitas score (ENG-01a), the next retention vector is **portability**: a disciplined trader applying to a prop firm, pitching their edge, or building a public identity benefits from a shareable credential. GitHub contribution graphs became career capital; Strava KOMs became athletic identity. A public profile at `/desk/d/<handle>` gives discipline the same surface.

This must be **strictly opt-in per element**. Default installation reveals nothing. Handle claim, score visibility, and sparkline visibility are all independent toggles. Absolutely no P&L, no balance, no position data, no trade history — discipline only.

---

## User Stories

- **As a trader applying to a prop firm / pitching my edge / building a public identity**, I want an opt-in public profile page showing my score + select stats, so my discipline becomes portable credential.
- **As a user who wants a unique name**, I want to claim a globally-unique handle (e.g. `0xwhale`, `cz`), so my profile URL is memorable.
- **As a privacy-first user**, I want the public profile to be strictly opt-in and fully revocable, so default installation reveals nothing.
- **As a user with a claimed handle who wants to regret it**, I want to release my handle and reclaim a new one, with rate limits that prevent churn abuse.

---

## Non-Goals

- **No streak counter on profile.** That is ENG-01c.
- **No vanity URL at `testudo.app/<handle>`.** The MVP route is `testudo.app/desk/d/<handle>`. Vanity subdomain is an ENG-02 concern.
- **No OpenGraph image generation.** MVP ships with client-side `document.title` only. OG image generator deferred to ENG-02 fast-follow.
- **No P&L, balance, position, or trade history on profile.** Discipline only.
- **No handle transfer between users.** Release + reclaim (by someone else) is allowed; direct transfer is not.
- **No handle review process.** First-come-first-served, with reserved-word list + profanity filter doing the gating. Abuse review is Phase 2 if needed.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Each user may claim one globally-unique handle (3–24 chars, `[a-z0-9_-]`). Reserved-word list enforced. Profanity filter enforced | High | backend |
| FR-2 | Handle claim: first-come-first-served. Release allowed. Change rate-limited to 1 per 30 days | High | backend |
| FR-3 | `POST /api/dignitas/handle`, `DELETE /api/dignitas/handle`, `PATCH /api/dignitas/visibility` endpoints | High | router |
| FR-4 | Public profile route `/desk/d/:handle` renders handle, optional bio (≤140 chars), join date, plus score/sparkline if opted-in | High | journal/frontend |
| FR-5 | `GET /api/public/profile/:handle` (no auth). Returns 404 for unclaimed. Respects each visibility flag independently (score null, sparkline null when off) | High | router |
| FR-6 | Profile visibility is **opt-in per element**: `show_score`, `show_sparkline`. Default all-off. Claimed-but-empty profile is a valid state (handle reservation) | High | backend + journal/frontend |
| FR-7 | `IdentitySettings.tsx` section on Account page: handle claim input, release button, visibility toggles, bio field | High | journal/frontend |
| FR-8 | Anti-abuse: public profile endpoint rate-limited per IP (60 req/min); handle claim/release rate-limited per user | Medium | router |
| FR-9 | `robots: noindex` on public profile by default. Opt-in to indexing via separate toggle in `IdentitySettings` | Medium | journal/frontend |
| FR-10 | Handle claimed with all visibility toggles off still reserves the handle (squatting is allowed for reservation; profile returns an empty carcass) | Medium | backend |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | `user_handles` migration + reserved-word list + profanity filter. Claim / release / visibility endpoints | Handles are unique, claimable, releasable, rate-limited |
| CP-2 | `IdentitySettings.tsx` on Account page — claim form, release button, visibility toggles, bio | User can manage public presence |
| CP-3 | Public profile `GET /api/public/profile/:handle` (no auth, visibility-respecting) + `/desk/d/:handle` route | Public shareable page works; nothing leaks without opt-in |
| CP-4 | Rate limits + `noindex` default + indexing opt-in | Anti-abuse baseline |

### Key Types

```rust
// crates/router/src/services/dignitas/handles.rs

pub struct UserHandle {
    pub user_id: Uuid,
    pub handle: String,          // normalized lowercase
    pub claimed_at: DateTime<Utc>,
    pub bio: Option<String>,     // ≤ 140 chars
    pub visibility: ProfileVisibility,
    pub allow_indexing: bool,    // default false
}

pub struct ProfileVisibility {
    pub show_score: bool,        // default false
    pub show_sparkline: bool,    // default false
}
```

```typescript
// testudo-journal/src/api/client.ts additions

export interface PublicProfile {
  handle: string
  bio: string | null
  score: string | null                                     // null if not opted-in
  sparkline: { date: string; score: string }[] | null      // null if not opted-in
  member_since: string
}

export interface IdentityPreferences {
  handle: string | null
  bio: string | null
  visibility: { show_score: boolean; show_sparkline: boolean }
  allow_indexing: boolean
  can_change_handle_at: string | null   // ISO 8601 or null if changeable now
}
```

### Handle Validation

- Regex: `^[a-z0-9][a-z0-9_-]{1,22}[a-z0-9]$` (3–24 chars, alphanum + `_`/`-`, must start/end alphanumeric)
- Normalized to lowercase before storage and lookup
- Reserved-word list in `testudo-journal/src/config/dignitas-reserved-handles.ts` (mirrored on backend in `services/dignitas/handles/reserved.rs`). Minimum coverage: `admin`, `testudo`, `api`, `www`, `root`, `support`, `help`, `mod`, `team`, `official`, plus major impersonation risks (`cz`, `sbf`, `vitalik`, etc. — configurable)
- Profanity filter: basic substring blocklist loaded at boot; not comprehensive, trip-wire only

### Files

**New (backend):**
- `crates/router/src/services/dignitas/handles/mod.rs`
- `crates/router/src/services/dignitas/handles/reserved.rs`
- `crates/router/src/services/dignitas/handles/profanity.rs`
- `crates/router/src/routes/public_profile.rs`
- `crates/router/tests/dignitas_handles_test.rs`
- `crates/sqlx_postgres/migrations/NNNN_user_handles.up.sql` + `.down.sql`

**New (frontend):**
- `testudo-journal/src/components/account/IdentitySettings.tsx`
- `testudo-journal/src/pages/PublicProfile.tsx`
- `testudo-journal/src/config/dignitas-reserved-handles.ts`

**Modified:**
- `crates/router/src/routes/dignitas.rs` (from ENG-01a) — add `/handle`, `/visibility` endpoints
- `crates/router/src/routes/mod.rs` — wire `public_profile` route (unauth)
- `testudo-journal/src/pages/Account.tsx` — include `IdentitySettings` section
- `testudo-journal/src/index.tsx` — register `/d/:handle` route (public, no auth guard)
- `testudo-journal/src/api/client.ts` — add handle + public profile surface
- `testudo-journal/src/components/DignitasPanel.tsx` (from ENG-01a) — add "Share profile" action, only visible when handle claimed + score opted-in

### Paved Roads

- **Existing auth middleware** — all endpoints auth-required except `/api/public/profile/:handle`
- **`PageSubHeader`, brutalist tokens** — profile page styling
- **`DignitasSparkline` from ENG-01a** — reused on public profile when `show_sparkline: true`
- **`dignitas_history` table from ENG-01a** — sole source for score on public profile

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Claim unique handle succeeds (201); claim taken handle returns 409; claim reserved word returns 400; claim invalid format returns 400
- [ ] Release handle succeeds; handle becomes claimable by other users
- [ ] Handle change rate-limit: second change within 30 days returns 429 with `can_change_handle_at` ISO timestamp
- [ ] Default claimed profile (all visibility off) returns 200 with `score: null, sparkline: null` — confirms opt-in, not opt-out
- [ ] Public profile `/desk/d/:handle` returns 404 for unclaimed handles
- [ ] Public profile respects each visibility toggle independently — score can be on, sparkline off, vice versa
- [ ] No auth required for `GET /api/public/profile/:handle`
- [ ] Profile rate-limited to 60 req/min per IP; 429 on breach
- [ ] `<meta name="robots" content="noindex">` on public profile HTML unless `allow_indexing: true`
- [ ] "Share profile" button in `DignitasPanel` copies `/desk/d/<handle>` to clipboard, only visible when handle claimed + score opted-in
- [ ] Verification: `cd testudo-exchange && cargo clippy --all-targets && cargo test`
- [ ] Verification: `cd testudo-journal && bun run build`
- [ ] Manual QA: one user with claimed handle + score opted-in, verified via incognito browser

---

## Risks

1. **Handle squatting.** Early users grab `0xwhale`, `cz`, etc. *Mitigation:* reserved-word list + 30-day rate limit + profanity filter. Accept residual squatting as the cost of frictionless onboarding. Review process is Phase 2.
2. **Public profile as gameable social pressure.** Users stop trading to preserve a high score rather than trade better. *Mitigation:* ENG-01a's score formula rewards journal consistency and setup adherence — sitting on hands produces a mediocre score, not a high one.
3. **Profanity filter false positives.** Blocklist-based filters false-positive on legitimate names. *Mitigation:* blocklist is trip-wire only — human can revert via a handle-override table if needed. Accept some false positives over a weak filter.
4. **SEO exposes user handles.** *Mitigation:* `robots: noindex` default. Indexing is a separate opt-in toggle.
5. **Rate-limiter on public endpoint is a DoS surface.** *Mitigation:* per-IP only, in-memory token bucket. If abuse becomes real, move to Redis-backed cluster-wide limiter. Initial naive impl is fine for MVP.

---

## Completion Signal

1. FR-1 through FR-10 implemented and tested
2. At least one user has claimed a handle and opted-in to a public profile visible at `/desk/d/<handle>` from an incognito browser
3. Handle rate-limit verified end-to-end
4. Verification commands pass
5. Committed: `feat(eng-01b): Dignitas public profile — handles, opt-in visibility, shareable discipline`
