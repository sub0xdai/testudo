# ENG-01b LEARNINGS

## T1 (2026-04-21)
Migration `20260421000001_user_handles.up.sql` lives in the submodule. Submodule commit is separate from parent pointer bump — two commits per task that touches the submodule (submodule first, then parent). Commit message for parent: "T1 — bump testudo-exchange submodule pointer".

## T2+T3+T4 (2026-04-21)
Router crate uses `lazy_static` for some Regex (coach/validator.rs) but `OnceLock` is also available and preferred per AGENTS.md. Both work; `OnceLock` was used for all three new static caches (HANDLE_RE, RESERVED, SUBSTRINGS).

`routes::auth::tests::test_me_returns_user_info` was pre-existing failing before this spec; confirmed via `git stash`. Not our regression.

`get_public_profile` normalises handle to lowercase before the DB lookup using `lower(handle) = $1` to match the partial unique index. Don't use `handle = $1` — the index is on `lower(handle)`, not on `handle`.

TOCTOU race window in `claim()` (EXISTS check + INSERT) is explicitly accepted at MVP scale per spec discovery. A unique constraint violation at INSERT would still surface as a DB error; the caller (route handler, T5) can map sqlx DB errors with constraint name matching if needed.

## T8 (2026-04-21)
`update_bio` was implemented in HandleService but had no HTTP route wired in T5. Added `PATCH /api/v1/dignitas/handle` (body `{ bio: string | null }`) in the same T8 iteration so `updateBio()` in the frontend client has a real endpoint. Bio endpoint returns 204 on success, 404 if no handle claimed, 400 if bio >140 chars.

`fetchPublicProfile()` uses bare `fetch` (no credentials) unlike all other client functions that use `fetchWithCredentials`. Returns `null` on 404 rather than throwing — callers render a 404 state instead of catching.

Error objects thrown by `claimHandle()` and `releaseHandle()` carry `.code`, `.status`, and `.data` fields so callers (IdentitySettings) can inspect the specific backend error code (e.g. `"rate_limited"`, `"handle_taken"`) without string-matching error messages.
