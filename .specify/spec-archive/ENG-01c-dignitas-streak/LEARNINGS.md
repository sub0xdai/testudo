# ENG-01c LEARNINGS

## Pre-CP-1 discovery (2026-04-22)

The spec's FR-2 assumed `coach_reports.severity` was a flat column. It is not — `coach_reports` is a week-grain row whose `digest_json` JSONB carries `flagged_patterns: [{ pattern, severity, evidence, metrics }]` per `services/coach/types.rs:93`. Severity enum: `Info | Notable | Concerning`. Only `Concerning` triggers a reset. The query uses `jsonb_path_exists(digest_json, '$.flagged_patterns[*] ? (@.severity == "Concerning")')` — scales fine at MVP because coach reports are weekly and per-user.

## CP-1 (2026-04-22) — service + migration + scheduler hook

Pure decision function `next_state(&StreakRow, Option<DateTime<Utc>>, now)` kept the reset semantic testable without a pool. Six unit tests covered fresh-user increment, no-flag increment, Concerning reset + longest trophy, longest-preservation on short-streak break, started_at anchoring, and wire-shape exposure. GREEN on first run because the semantic was crisp before writing code.

The `load_or_init` idempotent insert uses `ON CONFLICT (user_id) DO NOTHING` so the first tick for a never-ticked user is a no-op insert followed by the caller's own logic — avoids a race between two instances of the scheduler firing.

Scheduler hook is a per-user `apply_daily_tick` call after `take_daily_snapshot` with independent failure logging. Matches the snapshot failure pattern in `schedule::run_batch`; one user's streak tick failure does not abort the batch or affect any other user.

## CP-2 (2026-04-22) — /me extension + DignitasPanel render

Spec edge case 1 — "user with zero coach reports ever: streak: null" — required a per-request "has any coach_reports" EXISTS query in addition to the streak row fetch. The alternative (return days_clean = 0) would have surfaced a false counter on brand-new accounts before RSK-03 had any data on them. The null-when-no-coach-data semantic makes the `STREAK —` fallback in the UI meaningful: the user is told they have no streak yet, not that they have zero days.

DignitasPanel renders streak below the 90d sparkline with font-mono + text-tertiary styling; the `data-` prefixed label is "STREAK Nd  LONGEST Md" for opted-in users, `STREAK —` otherwise. No emoji, no icons, no animation (FR-8 non-goal gate).

## CP-3 (2026-04-22) — public profile opt-in

Migration split from the base `user_handles` migration: `20260422000001_add_show_streak` adds one column with `DEFAULT FALSE`. This lets it apply cleanly on already-migrated Postgres instances without touching the existing ENG-01b migration.

The `show_streak` column propagated through four places in `handles/mod.rs`: the `UserHandleRow` struct, every `SELECT` that reads the row (claim RETURNING, get_identity, get_public_profile), the `IdentityPreferences` wire struct, and the `VisibilityPatch` patch body. Missed one of these initially (claim RETURNING) — `cargo check` flagged it immediately because sqlx's `FromRow` derive doesn't compile-check column lists.

Public profile gate is `show_streak AND streak_row_exists`. Absent row returns `(None, None)` — consistent with the spec edge case where a fresh-claimed profile has no streak data yet.

## CP-4 (2026-04-22) — DB integration tests

`#[ignore]`-gated tests that require `DATABASE_URL`, matching the T15 rate-limit-integration pattern. Three tests lock in the AC matrix:

- Concerning → reset, longest_ever trophies the broken streak
- Same-day second tick → increment, NOT double-reset (the `generated_at > last_concerning_flag_at` gate holds in isolation)
- Info / Notable → increment, only Concerning resets

The `insert_coach_report` helper builds the full JSONB digest shape (`flagged_patterns[]` with pattern/severity/evidence/metrics). This also exercises the JSONB path query under production conditions — the pure unit tests cover decision logic but not SQL correctness.

## Frontend contract drift fix (pre-CP-1)

CP work was unblocked by fixing an ENG-01b shape-mismatch bug discovered during this session's manual QA: spec said `IdentityPreferences.visibility.show_score` (nested), backend shipped flat (`show_score` on root), T8 frontend followed spec. Mismatch caused a silent render throw on successful claim, trapping the UI in CLAIMING state. Fix flattened the frontend to match backend (matches DB columns too). Committed as `fix(eng-01b): flatten IdentityPreferences shape to match backend`.

Lesson: a spec-dictated wire shape has no teeth unless there's a test at the frontend/backend boundary that actually deserializes a real response. Add boundary-level contract tests for any wire type the frontend reads.

## Deferred manual QA (Completion Signal items 2–4)

Items 2 (two-week live observation across ≥ 3 users), 3 (at least one natural reset observed), and 4 (incognito verification of `show_streak`-opted-in profile) all require live DB + time + traffic. Not automatable. Mark as deferred; verify on the next live deploy window. CP-1 through CP-4 are code-complete; spec stays open until the deferred items resolve.

## Commits

- `5f68286` feat(eng-01c): CP-1 — streak service + migration + scheduler hook
- `fab7d64` feat(eng-01c): CP-1 — bump submodule
- `776ad29` feat(eng-01c): CP-2 — /me extension (FR-4)
- `2bdf6fb` feat(eng-01c): CP-2 — DignitasPanel render (FR-5)
- `4706854` feat(eng-01c): CP-3 — show_streak + public profile (FR-6)
- `3abc379` feat(eng-01c): CP-3 — frontend toggle + render
- `bd13a2c` test(eng-01c): CP-4 — DB integration tests
- `5065303` test(eng-01c): CP-4 — bump submodule
