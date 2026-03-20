# Specification: Minimal Journal Editor with Image Support and Markdown Export

**Spec ID:** UXP-06-journal-editor-refinement
**Date:** 2026-03-18
**Status:** Draft
**Class:** Feature / Journal
**Priority:** P1 — Journal is the app's differentiator; the editor must feel intentional, not bolted on
**Depends on:** UXP-01-design-system-alignment, UXP-04-motion-and-transitions
**Series:** UXP-01 through UXP-08 (Journal UX Polish from design critique)

---

## Problem Statement

The journal editor is a `max-w-3xl` centered modal competing with the data-dense aesthetic. It works, but it doesn't feel like a writing space. The markdown textarea is functional but spartan — no image support, no export capability, no "you're writing now" signal. Screenshots are critical for traders (chart annotations, trade setups, execution screenshots) but currently impossible to attach.

The fix: make the editor minimal and focused. When editing, dim the surrounding interface. Support image paste/upload with inline markdown preview. Add markdown export for portability.

---

## User Stories

- **As a trader**, I want to paste screenshots into my journal entries, so that I can attach chart setups and execution evidence.
- **As a trader**, I want to export entries as markdown files, so that I can back up my journal or share entries externally.
- **As a trader**, I want the editor to feel like a writing environment, so that I can focus on reflection without dashboard distractions.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Editor opens as a near-fullscreen overlay (`max-w-4xl`, `max-h-[95vh]`) with a dimmed background (`bg-black/80`) to create a focused writing environment | High | Editor |
| FR-2 | Support image paste (Ctrl+V with clipboard image) and file upload (click to browse or drag-and-drop onto textarea) | High | Editor |
| FR-3 | Images uploaded to backend via `POST /api/v1/journal/upload` endpoint returning a URL, inserted as `![description](url)` into markdown | High | API |
| FR-4 | Image preview renders inline in the PREVIEW tab via the existing MarkdownPreview component | High | Editor |
| FR-5 | Export button downloads the current entry as a `.md` file with YAML frontmatter (title, date, type, tags) and embedded image URLs | High | Editor |
| FR-6 | Export button also available on EntryCard (tertiary `[export]` action alongside `[edit]` and `[delete]`) | Medium | Timeline |
| FR-7 | Editor layout: metadata bar (type, title, tags, trade) as a compact horizontal strip at the top, full-width textarea/preview below — maximize writing surface | Medium | Editor |
| FR-8 | Textarea has a subtle left-border accent matching the entry type color (consistent with EntryCard pattern) | Low | Editor |
| FR-9 | Editor keyboard shortcut: `Ctrl+Enter` to save and close | Medium | Editor |

---

## Technical Implementation

### Image Upload Flow

```
User pastes/drops image
  → Convert to File/Blob
  → POST /api/v1/journal/upload (multipart/form-data)
  → Backend stores in configured path, returns { url: "/uploads/journal/{uuid}.png" }
  → Insert ![](url) at cursor position in textarea
  → Preview tab renders image inline
```

### Backend Upload Endpoint

```rust
// POST /api/v1/journal/upload
// Content-Type: multipart/form-data
// Returns: { "url": "/uploads/journal/{uuid}.{ext}" }

async fn upload_journal_image(
    mut payload: Multipart,
    auth: AuthenticatedUser,
) -> Result<HttpResponse, Error> {
    // Accept: PNG, JPG, WEBP, GIF
    // Max size: 5MB
    // Store in: ./uploads/journal/{uuid}.{ext}
    // Return URL path
}
```

### Paste Handler

```tsx
function handlePaste(e: ClipboardEvent) {
  const items = e.clipboardData?.items;
  if (!items) return;

  for (const item of items) {
    if (item.type.startsWith('image/')) {
      e.preventDefault();
      const file = item.getAsFile();
      if (file) uploadAndInsert(file);
    }
  }
}

async function uploadAndInsert(file: File) {
  const formData = new FormData();
  formData.append('file', file);
  const res = await fetch('/api/v1/journal/upload', {
    method: 'POST',
    headers: { Authorization: `Bearer ${getToken()}` },
    body: formData,
  });
  const { url } = await res.json();
  insertAtCursor(`![screenshot](${url})\n`);
}
```

### Markdown Export

```tsx
function exportEntry(entry: Entry) {
  const frontmatter = [
    '---',
    `title: "${entry.title}"`,
    `date: ${entry.created_at}`,
    `type: ${entry.entry_type}`,
    entry.tags?.length ? `tags: [${entry.tags.map(t => `"${t.name}"`).join(', ')}]` : null,
    '---',
    '',
  ].filter(Boolean).join('\n');

  const content = frontmatter + entry.body;
  const blob = new Blob([content], { type: 'text/markdown' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${entry.created_at.slice(0, 10)}-${slugify(entry.title)}.md`;
  a.click();
  URL.revokeObjectURL(url);
}
```

### Editor Layout (Revised)

```
┌──────────────────────────────────────────────┐
│ ┌──────────────────────────────────────────┐ │
│ │  [Type ▼]  Title________________  [Tags] │ │  ← Compact metadata strip
│ │  [Trade: BTC_USDT 2026-03-15]    [Save]  │ │
│ ├──────────────────────────────────────────┤ │
│ │                                          │ │
│ │  [EDIT] [PREVIEW]         [Export ↓]     │ │  ← Tab bar
│ │  ┃                                       │ │
│ │  ┃  Write your reflection here...        │ │  ← Left border accent
│ │  ┃                                       │ │
│ │  ┃  Paste images or drag files here.     │ │
│ │  ┃                                       │ │
│ │  ┃                                       │ │
│ │  ┃                                       │ │
│ │  ┃                                       │ │
│ │                                          │ │
│ ├──────────────────────────────────────────┤ │
│ │  Ctrl+Enter to save    [Close]           │ │  ← Footer
│ └──────────────────────────────────────────┘ │
│           (dimmed background)                │
└──────────────────────────────────────────────┘
```

### Files

- `testudo-journal/src/components/journal/EntryEditor.tsx` — Rework layout, add paste handler, add export button, add Ctrl+Enter
- `testudo-journal/src/components/journal/EntryCard.tsx` — Add `[export]` tertiary action
- `testudo-journal/src/components/journal/MarkdownPreview.tsx` — Ensure images render inline (should work already via `marked`)
- `testudo-journal/src/api/client.ts` — Add `uploadJournalImage()` function
- `testudo-exchange/crates/router/src/routes/journal.rs` — Add `upload_journal_image` handler
- `testudo-journal/src/lib/export.ts` — New markdown export utility

---

## Acceptance Criteria

- [ ] Pasting an image from clipboard inserts a markdown image tag at cursor position
- [ ] Dragging a file onto the textarea uploads and inserts it
- [ ] Image appears inline in PREVIEW tab
- [ ] Upload rejects files > 5MB with user-visible error
- [ ] Upload rejects non-image MIME types
- [ ] Export downloads a `.md` file with YAML frontmatter
- [ ] Export available from both editor and entry card
- [ ] Editor background dims surrounding interface (`bg-black/80`)
- [ ] `Ctrl+Enter` saves and closes the editor
- [ ] Editor has left-border accent matching entry type color
- [ ] `bun run build` passes and `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Image storage growth** — Mitigation: 5MB limit per upload, consider periodic cleanup of orphaned images (images not referenced by any entry).
2. **CORS on image URLs** — Mitigation: Serve uploads from the same origin via the Actix static file handler.
3. **Backend multipart parsing** — Mitigation: Actix-web has built-in `Multipart` extractor. Well-documented pattern.

---

## Completion Signal

This spec is complete when:
1. Images can be pasted and uploaded in the journal editor
2. Markdown export works with YAML frontmatter
3. Editor layout is compact and focused
4. Both frontend and backend build/test cleanly
5. Code committed to master
