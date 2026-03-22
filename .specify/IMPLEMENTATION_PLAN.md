# Implementation Plan

> Last updated: 2026-03-22
> Current spec: JNL-15-export-with-images
> Phase: BUILD

---

## Active Spec: JNL-15-export-with-images

Self-contained export with base64-embedded images + bulk export with progress indicator.

### Tasks

| ID | Task | Status | Complexity | Depends On |
|----|------|--------|------------|------------|
| T1 | Rewrite export.ts — inlineImages, blobToBase64, async exportEntry, buildFrontmatter, exportEntries + update callers for async | complete | medium | — |
| T2 | Add bulk export button + progress indicator to JournalTimeline.tsx | complete | low | T1 |
| T3 | Build validation + commit | complete | low | T2 |

### Key Decisions

- **exportEntry becomes async**: Callers don't need explicit error handling since `inlineImages` catches fetch failures internally. EntryCard wraps in void-returning arrow; EntryEditor awaits.
- **tagMap built from tradeDetailCache**: Bulk export reuses `getEntryTags()` to build the `Record<string, JournalTag[]>` map, no new API calls needed.
- **Progress as simple string signal**: `"3 / 12"` format shown inline next to Export All button during export. Clears on completion.

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

---

*This file is persistent state. Vox updates it each iteration.*
