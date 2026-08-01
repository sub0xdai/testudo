# Specification: Harden Markdown Preview and Editor UX

**Spec ID:** JNL-14-markdown-hardening
**Date:** 2026-03-22
**Status:** Draft
**Class:** Refactor / UX
**Priority:** P0 — Images overflow layout, scrollbars are invisible, editor discoverability gaps
**Depends on:** None (first in series)
**Series:** JNL-14 through JNL-18 (Journal audit remediation + database redesign)

---

## Problem Statement

The markdown preview CSS in `testudo-journal/src/styles/app.css` (lines 71-86) defines styles for h1-h6, p, strong, code, pre, blockquote, and links — but has **zero rules for `img` or `hr`**. Uploaded screenshots render at native resolution (1920px+), overflow the editor modal's `max-w-4xl` container, and break layout in both the preview pane and the entry card previews (`EntryCard.tsx` line 84).

Additionally, scrollbars are globally hidden (`scrollbar-width: none` on `*`, line 50-52), which means the editor textarea and preview pane — both scrollable containers — give no visual affordance that content extends below the fold. Users don't know they can scroll.

Finally, image upload only works via paste and drag-drop. There's no visible "attach image" button. The placeholder text ("Paste images or drag files here") disappears once the user starts typing, leaving no hint that upload exists.

---

## User Stories

- **As a trader**, I want pasted screenshots to display correctly in my journal entries, so that my visual notes are useful.
- **As a user**, I want to know when I can scroll in the editor, so that I don't think my content is truncated.
- **As a new user**, I want a visible way to attach images, so that I discover the upload feature without reading docs.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `.markdown-preview img` renders images constrained to container width with `max-width: 100%` and `height: auto` | High | app.css |
| FR-2 | `.markdown-preview img` has border and margin for visual separation from text | Medium | app.css |
| FR-3 | `.markdown-preview hr` renders as a visible horizontal rule using theme border color | Medium | app.css |
| FR-4 | Scrollbar hiding scoped to page body only; editor textarea and preview pane show native scrollbars | High | app.css |
| FR-5 | Visible "attach image" button in editor tab bar opens native file picker (`<input type="file" accept="image/*">`) | High | EntryEditor.tsx |
| FR-6 | Textarea allows vertical resize (`resize-y`) or auto-grows to fit content | Low | EntryEditor.tsx |

---

## Technical Implementation

### CSS Additions

```css
/* app.css — add to .markdown-preview block */
.markdown-preview img {
  max-width: 100%;
  height: auto;
  margin: 0.5rem 0;
  border: 1px solid rgb(var(--border));
}
.markdown-preview hr {
  border: none;
  border-top: 1px solid rgb(var(--border));
  margin: 1rem 0;
}
```

### Scrollbar Scoping

```css
/* app.css — replace global * scrollbar hide with body-only */
body {
  scrollbar-width: none;
}
body::-webkit-scrollbar { display: none; }
```

Remove lines 50-52 (`* { scrollbar-width: none }` and `*::-webkit-scrollbar { display: none }`).

### Attach Button

Add a hidden file input and trigger button in the tab bar area of EntryEditor.tsx:

```tsx
// In the tab bar div, alongside "Export .md" button
<label class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors cursor-pointer">
  Attach
  <input
    type="file"
    accept="image/*"
    class="hidden"
    onChange={(e) => {
      const file = e.currentTarget.files?.[0]
      if (file) uploadAndInsert(file)
      e.currentTarget.value = ''
    }}
  />
</label>
```

### Textarea Resize

Change `resize-none` to `resize-y` on the textarea class in EntryEditor.tsx line 359.

### Files

- `testudo-journal/src/styles/app.css` — add img/hr rules, scope scrollbar hiding
- `testudo-journal/src/components/journal/EntryEditor.tsx` — add attach button, allow resize

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Pasted screenshots display within container bounds (no overflow)
- [ ] Images have visible border and spacing in preview
- [ ] `---` markdown renders as a visible horizontal rule
- [ ] Editor textarea shows native scrollbar when content overflows
- [ ] Preview pane shows native scrollbar when content overflows
- [ ] Page-level scrollbar remains hidden
- [ ] "Attach" button opens file picker and inserts image on selection
- [ ] Textarea is vertically resizable
- [ ] `cd testudo-journal && bun run build` passes

---

## Risks

1. **Scrollbar appearance variation** — Native scrollbars look different across OS/browser. Mitigation: acceptable trade-off; visibility is more important than uniformity.
2. **Image border in dark theme** — 1px border may be too subtle on `#0a0a0a`. Mitigation: uses `rgb(var(--border))` which is already calibrated for the theme.

---

## Completion Signal

This spec is complete when:
1. Images render correctly in preview and entry cards
2. Scrollbars visible in editor/preview containers
3. Attach button functional
4. All acceptance criteria met
5. `bun run build` passes
6. Code committed to master
