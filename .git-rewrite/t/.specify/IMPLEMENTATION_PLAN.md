# Implementation Plan — ENG-01b Dignitas Public Profile

**Spec:** `.specify/specs/ENG-01b-dignitas-public-profile/spec.md`
**Depends on:** ENG-01a (complete; `dignitas_history` + `users.dignitas_pill_hidden` shipped)
**Strategy:** Four vertical checkpoints from the spec (CP-1..CP-4). Backend lands
first so the frontend has real endpoints to call; each CP is committable.

---

## Discoveries

- **Router crate is binary-only** — no `src/lib.rs`. Top-level `tests/…rs`
  cannot `use router::…`. The spec's `crates/router/tests/dignitas_handles_test.rs`
  is not viable. Tests live inline as `#[cfg(test)] mod tests` per module.
  Shared with `AGENTS.md`.
- **Rate limiter already exists.** `router::middleware::auth::RateLimiter`
  struct (per-IP, bucket over time window). Reuse the **struct** (no new
  dependency) — instantiate a **dedicated** `RateLimiter::new(60,
  Duration::from_secs(60))` for the public profile route. Do NOT share
  instance with the auth middleware's limiter, or the public-profile quota
  will couple to auth-endpoint traffic.
- **30-day handle-change window must survive a release.** If we store
  `last_changed_at` on `user_handles`, releasing deletes the row and the clock
  resets. Store `last_handle_change_at TIMESTAMPTZ` on `users` so the window
  persists across claim → release → reclaim by the same user.
- **Public route scope is new.** `/api/v1` today has `/auth` (mixed), `/depth`,
  `/dignitas` (JWT), etc. We add a sibling `web::scope("/public")` with no
  `JwtMiddleware` for the profile read.
- **Journal `Layout` wraps every route.** The `/d/:handle` route cannot depend
  on `AuthProvider`-gated fetches. Resolution: inside `PublicProfile.tsx`, do
  not call any authed client functions; use a bespoke `fetch` against
  `/api/v1/public/profile/:handle` without `credentials: 'include'`. If
  `Layout` itself fires authed probes (e.g. top-nav identity), exclude the
  public route via a conditional in `Layout` keyed on `useLocation`.
- **ENG-01a is already live.** `DignitasPanel`, `DignitasSparkline`,
  `dignitas_history` table, `/dignitas/me`, `/dignitas/history` all exist.
  The public profile is a server-side read of the same data with the
  visibility gate applied; we do not reuse the authed handlers (they require
  `AuthenticatedUser`). We build a dedicated read path that takes a handle
  and joins through `user_handles → dignitas_history`.
- **Path prefix.** Spec writes `/api/public/profile/:handle`; the real server
  base is `/api/v1`, so the canonical path is
  `/api/v1/public/profile/:handle`. Documented explicitly here so FR-5 is not
  ambiguous.
- **`<meta name="robots">` in an SPA — two-layer defence.** Vite builds a
  single `index.html`; imperative meta-tag management covers JS-executing
  crawlers (Googlebot) but NOT static HTML scrapers (Archive.org, Facebook
  OG, many SEO tools). FR-9 requires noindex default; a single-layer JS-only
  fix leaks profiles to every non-JS crawler. **Resolution (two layers):**
  1. **HTTP header layer (Cloudflare Pages `_headers`):** serve
     `X-Robots-Tag: noindex, nofollow` for `/d/*` — covers all crawlers
     regardless of JS execution. Applied at the CDN edge; see
     `reference_cloudflare_pages` memory for current Pages deploy flow.
     Documented under T12b.
  2. **SPA layer (`PublicProfile.tsx`):** imperative meta-robots management
     via `onMount`/`onCleanup` — default `noindex, nofollow`, flips to
     `index, follow` when `allow_indexing === true`. Overridden header MUST
     also flip server-side when the underlying profile opts in — the
     `_headers` file can't see the DB, so the API response shape is
     orthogonal; header stays noindex by default, the SPA-side meta flips
     when the API returns `allow_indexing: true`, and Googlebot honors the
     stricter of the two. True indexing (archive.org, static scrapers)
     stays off until we add per-handle header rewriting in a later spec.
     This is acceptable MVP — "opt-in to JS-crawler indexing" is stronger
     than "opt-in to full web indexing."
- **Reserved-word mirror.** The spec requires parallel lists on backend and
  frontend. Treat the backend file as canonical (the server is the
  enforcement gate); the frontend file exists only for pre-submit UX hints.
  Backend always re-validates.
- **CORS for new methods.** `PATCH` is already enabled project-wide (prior
  CORS fix for coach preferences). `DELETE` is in use elsewhere. No CORS
  changes needed.

---

## Tasks

### CP-1 — Backend foundation: handles table, validation, service

- [ ] **T1** — Migration `NNNN_user_handles.up.sql` + `.down.sql`. Creates
  `user_handles(user_id UUID PK → users ON DELETE CASCADE, handle TEXT UNIQUE
  NOT NULL, bio TEXT NULL, show_score BOOLEAN NOT NULL DEFAULT FALSE,
  show_sparkline BOOLEAN NOT NULL DEFAULT FALSE, allow_indexing BOOLEAN NOT
  NULL DEFAULT FALSE, claimed_at TIMESTAMPTZ NOT NULL DEFAULT NOW())`.
  `CREATE UNIQUE INDEX idx_user_handles_handle_lower ON user_handles
  (lower(handle))` for case-insensitive uniqueness. Adds
  `last_handle_change_at TIMESTAMPTZ NULL` column to `users` for the 30-day
  window that survives release. *Complexity: simple.*

- [ ] **T2** — `services/dignitas/handles/validation.rs`: pure
  `validate_handle(&str) -> Result<NormalizedHandle, HandleError>` — trims,
  lowercases, enforces regex `^[a-z0-9][a-z0-9_-]{1,22}[a-z0-9]$`, checks
  reserved list, runs profanity filter. Inline `#[cfg(test)]` covers: valid,
  too short, too long, leading `-`, trailing `_`, uppercase normalization,
  reserved hit, profanity hit. RED first, then GREEN. *Complexity: simple.*

- [ ] **T3** — `services/dignitas/handles/reserved.rs` +
  `services/dignitas/handles/profanity.rs`: `static` lists loaded via
  `OnceLock<HashSet<&'static str>>`. Reserved minimum set per spec: `admin`,
  `testudo`, `api`, `www`, `root`, `support`, `help`, `mod`, `team`,
  `official`, `cz`, `sbf`, `vitalik`. Profanity: 10–20 core substrings (not
  exhaustive — trip-wire only, per spec risk 3). Inline unit tests for
  lookup, substring match. *Complexity: simple.*

- [ ] **T4** — `services/dignitas/handles/mod.rs`: `HandleService` with
  postgres-backed methods — `claim(user_id, handle, bio) ->
  Result<ClaimOutcome, HandleError>` (enforces 30-day window via
  `users.last_handle_change_at`, returns `Err::RateLimited { retry_at }`,
  `Err::Taken`, `Err::Reserved`, `Err::Invalid`), `release(user_id)`,
  `update_visibility(user_id, patch)`, `update_bio`, `update_indexing`,
  `get_identity(user_id) -> IdentityPreferences`,
  `get_public_profile(handle) -> Option<(UserHandle, Option<ScoreRow>,
  Option<Vec<HistoryRow>>)>` with visibility-gated fetches. All DB writes in
  one transaction per mutation; bump `last_handle_change_at` on claim and
  release. Inline pure tests for outcome decisions where possible; DB-bound
  paths covered by an `#[ignore]` integration test gated on `DATABASE_URL`.
  *Complexity: medium.*

### CP-1 continued — Authed endpoints

- [ ] **T5** — `routes/dignitas.rs` additions:
  `POST /api/v1/dignitas/handle` (body: `{ handle, bio? }` → 201 +
  IdentityPreferences on success; 400 invalid; 409 taken; 400 reserved; 429
  w/ `{ can_change_handle_at }` on rate limit).
  `DELETE /api/v1/dignitas/handle` (204 on success; 429 w/
  `can_change_handle_at` on rate limit; 404 if no handle currently claimed).
  `PATCH /api/v1/dignitas/visibility` (body `{ show_score?, show_sparkline?,
  allow_indexing? }` → 204). `GET /api/v1/dignitas/identity` (returns
  `IdentityPreferences`: handle, bio, visibility, allow_indexing,
  can_change_handle_at). Wire into the existing `/dignitas` scope in
  `main.rs`. Request validation tests inline. *Complexity: simple.*

### CP-3 backend — Public read + rate limit

- [ ] **T6** — `routes/public_profile.rs`:
  `GET /api/v1/public/profile/:handle` (no JWT) using the handle service.
  404 when unclaimed. 200 with `{ handle, bio, score: null|string,
  sparkline: null|[...], member_since }` — `score` null unless
  `show_score=true`, `sparkline` null unless `show_sparkline=true`.
  Per-IP rate limit via a **dedicated** `RateLimiter::new(60,
  Duration::from_secs(60))` (separate instance from auth middleware's)
  stored in `app_data`. 429 on breach. Record attempt on every call.
  **Inline tests enumerate the full visibility matrix — FR-10 empty-carcass
  coverage:** (a) `{show_score:false, show_sparkline:false}` → 200 with
  both null (FR-10), (b) `{true, false}` → score populated, sparkline
  null, (c) `{false, true}` → score null, sparkline populated, (d)
  `{true, true}` → both populated. Plus rate-limit wiring test (mock IP).
  *Complexity: simple.*

- [ ] **T7** — `main.rs` wiring: create the public profile `RateLimiter`,
  add `routes::public_profile` to `routes/mod.rs`, register
  `web::scope("/public").service(web::scope("/profile").route("/{handle}",
  web::get().to(public_profile::get_profile)))` as a sibling of `/dignitas`
  with **no** `JwtMiddleware`. *Complexity: simple.*

### CP-2 frontend — Account identity management

- [ ] **T8** — `testudo-journal/src/api/client.ts` additions:
  `IdentityPreferences`, `PublicProfile` types; `fetchIdentity()`,
  `claimHandle()`, `releaseHandle()`, `patchVisibility()`,
  `patchIndexing()`, `updateBio()` (authed — via `fetchWithCredentials`);
  `fetchPublicProfile(handle)` (unauth — bare `fetch`, no credentials).
  *Complexity: simple.*

- [ ] **T9** — `testudo-journal/src/config/dignitas-reserved-handles.ts`
  mirroring the backend's minimum reserved list. UX-only; backend is the
  enforcement gate. *Complexity: trivial.*

- [ ] **T10** — `components/account/IdentitySettings.tsx`: brutalist
  section on Account page. Claim form with inline regex/reserved preview
  (fires backend call on submit — trusts server as final gate). Release
  button with confirm. Bio `<textarea>` (≤140 chars, countdown). Three
  toggles: `show_score`, `show_sparkline`, `allow_indexing`. Shows
  `can_change_handle_at` timer when rate-limited. Uses the existing
  `PageSubHeader` styling and `HelpTip` conventions. *Complexity: medium.*

- [ ] **T11** — `pages/Account.tsx`: mount `<IdentitySettings />` as a new
  section (below `CoachBanner`). *Complexity: trivial.*

### CP-3 frontend — Public profile page

- [ ] **T12** — `pages/PublicProfile.tsx`: reads `:handle` from route
  params, calls `fetchPublicProfile`, renders 404 state on null, otherwise
  shows brutalist header with handle + join date + bio, plus the score
  card + 90-day `DignitasSparkline` only when opted-in. Manages
  `<meta name="robots">` in `onMount` / `onCleanup` — defaults noindex,
  flips to `index, follow` only when `allow_indexing: true` is returned.
  Sets `document.title` to `Testudo — /d/<handle>`. No authed calls.
  *Complexity: medium.*

- [ ] **T13** — `index.tsx`: register `<Route path="/d/:handle"
  component={PublicProfile} />`. Verify `Layout` does not fire authed
  probes for this path — if it does (top-nav identity etc.), gate them
  off via `useLocation().pathname.startsWith('/d/')`. *Complexity: simple.*

- [ ] **T13b** — **HTTP-header FR-9 enforcement (Cloudflare Pages).**
  Add a `_headers` file entry (or update existing) under
  `testudo-journal/public/_headers`:
  ```
  /d/*
    X-Robots-Tag: noindex, nofollow
  ```
  Ensures static HTML crawlers (Archive.org, Facebook OG, non-JS SEO
  bots) are blocked from indexing public profiles regardless of the SPA
  imperative meta layer. Complements T12's JS-layer control. Verify via
  `curl -I https://testudo.app/d/test-handle | grep -i x-robots-tag`
  after deploy. Manual QA (T15/T16): confirm header present on a real
  profile URL. *Complexity: trivial.*

- [ ] **T14** — `components/DignitasPanel.tsx`: add a third action
  "SHARE PROFILE" that copies `${origin}/desk/d/<handle>` to clipboard.
  Only rendered when identity has a claimed handle AND `show_score` is
  true (we require a minimally-visible profile before offering the share
  button — squatted-empty profiles are URL-shareable but the pill panel
  is not the right surface). Fetch identity via `createResource` against
  `fetchIdentity`. Toast on copy success. *Complexity: simple.*

### CP-4 — Anti-abuse finish + verification

- [ ] **T15** — Integration sweep for rate-limit paths:
  `#[tokio::test] #[ignore]` DB-backed test for handle claim hitting
  30-day window; inline unit test for per-IP 60/min breach returning 429.
  Document the manual-QA deferral (incognito browser verification) in the
  spec's completion signal. *Complexity: simple.*

- [ ] **T16** — Verification pass: `cd testudo-exchange && cargo clippy
  --all-targets && cargo test`; `cd testudo-journal && bun run build`.
  Fix any regressions. *Complexity: simple.*

- [ ] **T17** — Append to `.specify/specs/ENG-01b-dignitas-public-profile/
  LEARNINGS.md` with per-task gotchas (at minimum: inline-test override,
  rate-limit sharing, `last_handle_change_at` location, SPA meta-robots
  handling). Final commit with
  `feat(eng-01b): Dignitas public profile — handles, opt-in visibility,
  shareable discipline`. *Complexity: trivial.*

---

## Commit strategy

- T1 alone — migration (reversible, isolated).
- T2 + T3 + T4 bundled — pure-module + service are useless individually; no
  callers before T5. Prevents broken-intermediate state per `AGENTS.md`.
- T5 bundled with its `main.rs` wiring — authed handle/visibility
  endpoints ship + smoke-testable independently (JWT curl works).
- T6 bundled with T7 (public scope wiring) — unauth endpoint + dedicated
  RateLimiter ship together. Landing AFTER T5 means the authed path is
  already curl-verified before the public surface appears, narrowing blast
  radius if anything regresses.
- T8 alone — pure API surface additions; no callers yet.
- T9, T10, T11 bundled — frontend Account-page slice.
- T12 + T13 + T13b bundled — public profile page + route registration + `_headers` FR-9 enforcement (ships as one "public profile slice" atomic change).
- T14 alone — Dignitas panel enhancement.
- T15 alone — test hardening.
- T16 alone — verification fixes (if any).
- T17 alone — LEARNINGS + final spec-complete commit.

---

## Blockers

None. All dependencies (ENG-01a, `RateLimiter`, `users` table, journal
routing, Account page scaffold) are in place.

---

## PLANNING COMPLETE

Spec: ENG-01b-dignitas-public-profile
Total Tasks: 18 (T1–T17 + T13b)
Ready for BUILD mode — Gate 1 pass 2026-04-21.

Next task: T1 — `user_handles` migration + `users.last_handle_change_at` column.
