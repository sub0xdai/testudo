# Implementation Plan

> Last updated: 2026-03-22
> Current spec: JNL-18-storage-quotas
> Phase: BUILD

---

## Active Spec: JNL-18-storage-quotas

Per-user storage quotas for journal images — track cumulative bytes, enforce caps, provide UI for monitoring and management.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Database migration for `journal_images` table + Rust model (`JournalImage` struct in `models/journal.rs`) | complete | low | — |
| T2 | Backend endpoints — modify `upload_journal_image` with quota check + DB insert, add `GET /journal/storage`, add `DELETE /journal/images/:id` | complete | medium | T1 |
| T3 | Frontend — add `fetchStorageUsage()` and `deleteImage()` to API client, create `StorageBar.tsx` component, handle quota error in `EntryEditor.tsx`, integrate StorageBar into journal layout | complete | medium | T2 |
| T4 | Build validation (both `cargo clippy && cargo test` and `bun run build`) + commit | complete | low | T3 |

### Key Decisions

- **Filesystem storage unchanged**: Images remain on filesystem at `./uploads/journal/`. The new `journal_images` table tracks metadata (size, path, user) for quota enforcement — it doesn't change how files are stored.
- **DB row before file write**: Insert the `journal_images` row first (with file_size), then write the file. Rollback DB row on write failure. Prevents quota desync (spec Risk #3).
- **Default 100MB quota**: Hardcoded constant `DEFAULT_QUOTA_BYTES: i64 = 100 * 1024 * 1024`. No per-user override table yet (FR-8 is Low priority).
- **AppState pool access**: All journal routes already use `app_state: web::Data<AppState>` → `app_state.pool` pattern. New endpoints follow the same pattern.
- **ErrorResponse with details**: Quota exceeded returns `ErrorResponse::with_details("quota_exceeded", message, { used_bytes, quota_bytes, remaining_bytes })` so the frontend can show specific numbers.
- **No backfill migration**: Spec Risk #2 mentions backfilling existing images. Deferred — existing uploads will be "free" (not tracked). Only new uploads count against quota.
- **Image deletion removes file + DB row**: `DELETE /journal/images/:id` deletes both the filesystem file and the DB row. Frontend shows warning about orphaned markdown references (spec Risk #1).

---

## Completed Specs

| Spec | Completion Date |
|------|-----------------|
| HL-11-status-transition-fix | 2026-03-21 |
| UXP-18-multi-theme | 2026-03-21 |
| EXT-37-message-dispatch-refactor | 2026-03-22 |
| EXT-38-background-decomposition | 2026-03-22 |
| UXP-19-features-layout | 2026-03-22 |
| UXP-22-signal-color-calibration | 2026-03-22 |
| UXP-20-strip-glassmorphism | 2026-03-22 |
| UXP-23-landing-typography | 2026-03-22 |
| UXP-21-light-theme-parity | 2026-03-22 |
| JNL-14-markdown-hardening | 2026-03-22 |
| JNL-15-export-with-images | 2026-03-22 |
| JNL-16-database-view | 2026-03-22 |
| JNL-17-nested-collections | 2026-03-22 |

---

*This file is persistent state. Vox updates it each iteration.*
