# Specification: Self-Contained Export with Embedded Images

**Spec ID:** JNL-15-export-with-images
**Date:** 2026-03-22
**Status:** Draft
**Class:** Feature / Data Portability
**Priority:** P1 — Current export produces broken `.md` files when server URLs change or go offline
**Depends on:** JNL-14-markdown-hardening
**Series:** JNL-14 through JNL-18 (Journal audit remediation + database redesign)

---

## Problem Statement

The current `exportEntry()` function in `testudo-journal/src/lib/export.ts` writes raw markdown to a `.md` blob download. Image references in the body (e.g., `![screenshot](http://server/uploads/abc.png)`) are left as remote URLs pointing to the backend. This means:

1. The exported file is **not self-contained** — it depends on the server being accessible.
2. If the backend rotates storage, migrates, or goes down, all exported `.md` files have broken images.
3. Users expect "export" to mean "take my data with me" — remote URLs violate that expectation.

Additionally, there is no bulk export. A user with 50 entries must click `[Export]` on each card individually.

---

## User Stories

- **As a trader**, I want my exported journal entries to include images inline, so that the export works offline and survives server changes.
- **As a user**, I want to export all my filtered entries at once, so that I can back up my journal efficiently.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Single-entry export fetches all images referenced in the body, converts to base64 data URIs, and replaces the URLs in the exported markdown | High | export.ts |
| FR-2 | Export produces a self-contained `.md` file that renders correctly in any markdown viewer (VS Code, Obsidian, Typora, GitHub) | High | export.ts |
| FR-3 | Bulk export button in JournalTimeline exports all currently filtered entries as a single `.md` file with entries separated by `---` | Medium | JournalTimeline.tsx, export.ts |
| FR-4 | Bulk export includes YAML frontmatter for each entry | Medium | export.ts |
| FR-5 | Export shows progress indicator for image fetching | Low | EntryEditor.tsx, JournalTimeline.tsx |

---

## Technical Implementation

### Image Inlining

```typescript
// export.ts — new helper
async function inlineImages(markdown: string): Promise<string> {
  const imageRegex = /!\[([^\]]*)\]\(([^)]+)\)/g
  const matches = [...markdown.matchAll(imageRegex)]

  let result = markdown
  for (const match of matches) {
    const [full, alt, url] = match
    if (url.startsWith('data:')) continue // already inline
    try {
      const res = await fetch(url)
      const blob = await res.blob()
      const base64 = await blobToBase64(blob)
      result = result.replace(full, `![${alt}](${base64})`)
    } catch {
      // Leave original URL if fetch fails
    }
  }
  return result
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve) => {
    const reader = new FileReader()
    reader.onloadend = () => resolve(reader.result as string)
    reader.readAsDataURL(blob)
  })
}
```

### Updated exportEntry

```typescript
export async function exportEntry(entry: JournalEntry, tags?: JournalTag[]) {
  const frontmatter = buildFrontmatter(entry, tags)
  const body = await inlineImages(entry.body)
  const content = frontmatter + '\n' + body
  downloadMarkdown(content, entry)
}
```

Note: `exportEntry` becomes async. Callers need to await it.

### Bulk Export

```typescript
export async function exportEntries(
  entries: JournalEntry[],
  tagMap: Record<string, JournalTag[]>,
  onProgress?: (current: number, total: number) => void,
): Promise<void> {
  const sections: string[] = []
  for (let i = 0; i < entries.length; i++) {
    const entry = entries[i]
    const tags = tagMap[entry.id] ?? []
    const frontmatter = buildFrontmatter(entry, tags)
    const body = await inlineImages(entry.body)
    sections.push(frontmatter + '\n' + body)
    onProgress?.(i + 1, entries.length)
  }
  const content = sections.join('\n\n---\n\n')
  const blob = new Blob([content], { type: 'text/markdown' })
  downloadBlob(blob, `testudo-journal-export-${new Date().toISOString().slice(0, 10)}.md`)
}
```

### Files

- `testudo-journal/src/lib/export.ts` — rewrite with image inlining + bulk export
- `testudo-journal/src/components/journal/EntryEditor.tsx` — make export call async
- `testudo-journal/src/components/journal/EntryCard.tsx` — make export call async
- `testudo-journal/src/components/journal/JournalTimeline.tsx` — add bulk export button

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Single-entry export produces `.md` with base64-embedded images
- [ ] Exported `.md` renders correctly in VS Code markdown preview
- [ ] Images that fail to fetch are left as original URLs (graceful degradation)
- [ ] Bulk export button visible in JournalTimeline header
- [ ] Bulk export includes all currently filtered entries
- [ ] Each entry in bulk export has YAML frontmatter
- [ ] Progress indicator shows during multi-entry export
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Large file size** — Base64-encoded images are ~33% larger than binary. A journal with 20 screenshots could produce a 50MB+ file. Mitigation: acceptable for backup/export; this is the user's data they're taking with them. Consider warning if total size exceeds 100MB.
2. **CORS on image fetch** — If uploaded images are served from a different origin, fetch may fail. Mitigation: images are served from the same API origin; graceful fallback leaves original URL.
3. **Async migration** — `exportEntry` becomes async, requiring caller updates. Mitigation: only 2 call sites (EntryCard and EntryEditor), both straightforward.

---

## Completion Signal

This spec is complete when:
1. Single-entry export embeds images as base64
2. Bulk export functional with progress indicator
3. All acceptance criteria met
4. `bun run build` passes
5. Code committed to master
