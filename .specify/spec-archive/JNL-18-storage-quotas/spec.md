# Specification: Per-User Storage Quotas for Journal Images

**Spec ID:** JNL-18-storage-quotas
**Date:** 2026-03-22
**Status:** Draft
**Class:** Feature / Infrastructure
**Priority:** P2 — Required before public release to prevent unbounded storage growth
**Depends on:** JNL-15-export-with-images (users need export before we limit them)
**Series:** JNL-14 through JNL-18 (Journal audit remediation + database redesign)

---

## Problem Statement

The journal image upload endpoint (`POST /api/v1/journal/upload`) currently accepts any image up to 5MB with no per-user cumulative limit. Each uploaded image is stored server-side and served indefinitely. Without quotas:

1. A single user could upload thousands of screenshots, consuming gigabytes of storage.
2. There's no visibility into how much storage a user has consumed.
3. There's no mechanism to encourage users to export and clean up old data.
4. Hosting costs scale linearly with user count × upload volume with no ceiling.

The solution is a per-user storage quota system: track cumulative bytes uploaded, enforce a cap (e.g., 100MB free tier, higher for paid), and provide UI for monitoring usage and managing stored images.

---

## User Stories

- **As a platform operator**, I want per-user storage limits, so that hosting costs are predictable and bounded.
- **As a user**, I want to see how much storage I've used, so that I can manage my uploads and export before hitting the limit.
- **As a user**, I want a clear error message when I hit my quota, so that I know to export and delete old entries to free space.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Backend tracks cumulative bytes uploaded per user | High | Backend API |
| FR-2 | Upload endpoint rejects files that would exceed the user's quota with a clear error message | High | Backend API |
| FR-3 | `GET /journal/storage` returns `{ used_bytes, quota_bytes, image_count }` | High | Backend API |
| FR-4 | Journal UI shows storage usage bar (used/quota) in the page header or sidebar | High | Journal page |
| FR-5 | Upload rejection error in EntryEditor shows remaining space and suggests export | High | EntryEditor.tsx |
| FR-6 | Users can delete individual images to reclaim space | Medium | Backend API + UI |
| FR-7 | Default quota is 100MB for free tier | High | Backend config |
| FR-8 | Quota is configurable per user (for future paid tiers) | Low | Backend config |

---

## Technical Implementation

### Backend Storage Tracking

[CLARIFY] How are uploaded images currently stored? Filesystem? S3? The storage tracking approach depends on this:

**Option A — Database tracking (preferred):**
```sql
CREATE TABLE journal_images (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id),
  file_name TEXT NOT NULL,
  file_size BIGINT NOT NULL,        -- bytes
  mime_type TEXT NOT NULL,
  storage_path TEXT NOT NULL,        -- relative path or S3 key
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_journal_images_user ON journal_images(user_id);
```

**Upload endpoint change:**
```rust
// Before accepting upload:
let used = sqlx::query_scalar!(
    "SELECT COALESCE(SUM(file_size), 0) FROM journal_images WHERE user_id = $1",
    user_id
).fetch_one(&pool).await?;

let quota = get_user_quota(user_id); // default: 100MB
if used + file_size > quota {
    return Err(ApiError::QuotaExceeded {
        used,
        quota,
        remaining: quota - used,
    });
}
```

### Storage Usage API

```
GET /api/v1/journal/storage

Response:
{
  "used_bytes": 42_567_890,
  "quota_bytes": 104_857_600,    // 100MB
  "image_count": 23
}
```

### Frontend Storage Bar

```tsx
// src/components/journal/StorageBar.tsx
function StorageBar(props: { used: number; quota: number }) {
  const pct = () => Math.min(100, (props.used / props.quota) * 100)
  const label = () => `${formatBytes(props.used)} / ${formatBytes(props.quota)}`

  return (
    <div class="flex items-center gap-2">
      <div class="flex-1 h-1.5 bg-container-bg border border-container-border overflow-hidden">
        <div
          class="h-full transition-all"
          classList={{
            'bg-signal-green/60': pct() < 70,
            'bg-signal-amber/60': pct() >= 70 && pct() < 90,
            'bg-signal-red/60': pct() >= 90,
          }}
          style={{ width: `${pct()}%` }}
        />
      </div>
      <span class="font-mono text-[10px] text-text-tertiary whitespace-nowrap">{label()}</span>
    </div>
  )
}
```

### Upload Rejection UX

When the upload endpoint returns a quota error, the EntryEditor should display:

```
Storage limit reached (95.2MB / 100MB). Export your entries to free up space.
```

With a link/button to trigger bulk export.

### Image Deletion

```
DELETE /api/v1/journal/images/:id

// Removes file from storage, deletes DB row, reclaims quota
```

Frontend: add a "Manage Storage" view listing uploaded images with delete buttons and file sizes. Accessible from the storage bar.

### Files

- Backend: new migration for `journal_images` table
- Backend: update upload endpoint with quota check
- Backend: new `GET /journal/storage` and `DELETE /journal/images/:id` endpoints
- `testudo-journal/src/api/client.ts` — add `fetchStorageUsage()` and `deleteImage()` functions
- `testudo-journal/src/components/journal/StorageBar.tsx` — **new** — usage indicator
- `testudo-journal/src/components/journal/EntryEditor.tsx` — handle quota error in upload
- `testudo-journal/src/pages/Journal.tsx` — show StorageBar in header/sidebar

### Dependencies Added

None (frontend). Backend: no new crates.

---

## Acceptance Criteria

- [ ] Upload endpoint rejects files that would exceed user quota
- [ ] Rejection error message includes used/quota/remaining bytes
- [ ] Storage bar visible in journal UI showing used/quota
- [ ] Storage bar color changes at 70% (amber) and 90% (red) thresholds
- [ ] Users can delete individual images to reclaim space
- [ ] Quota defaults to 100MB for new users
- [ ] EntryEditor shows clear guidance when quota is reached
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Image deletion orphans markdown references** — Deleting an image file breaks `![screenshot](url)` references in entry bodies. Mitigation: show warning before delete listing affected entries; or tombstone the image with a "deleted" placeholder.
2. **Migration for existing users** — Users who have already uploaded images won't have `journal_images` rows. Mitigation: migration script scans storage directory/S3 and back-fills the table.
3. **Quota bypass** — If tracking is only in the database, a failed insert after successful file write could desync. Mitigation: insert DB row first (with file_size), then write file; rollback DB row on write failure.

---

## Completion Signal

This spec is complete when:
1. Upload quota enforced on backend
2. Storage usage visible in journal UI
3. Image deletion functional
4. All acceptance criteria met
5. Both frontend and backend builds pass
6. Code committed to master
