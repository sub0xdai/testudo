# Specification: {Title — Short Imperative Description}

**Spec ID:** {SERIES-NN-slug}
**Date:** {YYYY-MM-DD}
**Status:** Draft
**Class:** {Infrastructure | Feature | Core | Testing | Refactor} / {Subclass}
**Priority:** {P0 | P1 | P2} — {one-line justification}
**Depends on:** {Spec IDs or "None (first in series)"}
**Series:** {SERIES-NN through SERIES-NN (series description)}

---

## Problem Statement

{2-3 paragraphs. State the problem concretely — what's broken, missing, or needed.
Include root cause analysis. Reference specific files, errors, or behaviors.
End with why this spec is the right fix.}

---

## User Stories

- **As a {role}**, I want {capability}, so that {benefit}.
- **As a {role}**, I want {capability}, so that {benefit}.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | {Specific, testable requirement} | High | {Module} |
| FR-2 | {Specific, testable requirement} | High | {Module} |
| FR-3 | {Specific, testable requirement} | Medium | {Module} |

---

## Technical Implementation

### Vertical Checkpoints

Break implementation into vertical slices. Each checkpoint is independently testable and committable.

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | {End-to-end slice with mock/minimal data} | {What you can test} |
| CP-2 | {Real backend/data for CP-1 slice} | {Integration test} |
| CP-3 | {Next slice end-to-end} | {What you can test} |

### {Section 1 — e.g. Key Types, Architecture}

{Describe the design. Include code blocks for key types, structs, interfaces. LEAD WITH TDD, ensure test coverage}

```rust
// or typescript, bash, etc.
pub struct ExampleStruct {
    field: Type,
}
```

### {Section 2 — e.g. Method Mapping, Integration Points}

{Tables work well for mapping decisions.}

| Source | Target | Notes |
|--------|--------|-------|
| ... | ... | ... |

### Paved Roads

{Existing patterns, libraries, or conventions discovered during research that this spec reuses. Reference specific files.}

### Files

- `path/to/new_file.rs` — {purpose}
- `path/to/modified_file.rs` — {what changes}

### Dependencies Added

- `crate = "version"` — {why needed}

---

## Acceptance Criteria

- [ ] {Criterion directly tied to an FR}
- [ ] {Criterion directly tied to an FR}
- [ ] {Error path tested}
- [ ] {Verification command passes: `cargo clippy --all-targets && cargo test` or `bun run build`}

---

## Risks

1. **{Risk name}** — {description}. Mitigation: {how to handle it}.
2. **{Risk name}** — {description}. Mitigation: {how to handle it}.

---

## Completion Signal

This spec is complete when:
1. {Key deliverable implemented and tested}
2. {All acceptance criteria met}
3. {Verification commands pass}
4. {Code committed to master}
