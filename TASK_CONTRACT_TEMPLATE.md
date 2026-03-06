# Task Contract: [TASK NAME]

## Spec Reference
- Specification: `.specify/specs/[NNN]-[name]/spec.md`
- Related specs: (list any dependencies)

---

## 1. Implementation Details

### Scope
- **Files to modify**: (exact paths)
- **Files to create**: (exact paths)
- **Files NOT to touch**: (boundaries)

### Requirements
- [ ] FR-1: (functional requirement)
- [ ] FR-2: ...

### Technical Approach
(Brief description of the implementation strategy — what pattern, what abstraction, what trade-offs)

---

## 2. Deterministic Test Requirements

### Tests to Write
- [ ] `test_name_1`: (what it verifies)
- [ ] `test_name_2`: (what it verifies)

### Tests to Pass
```bash
# Must pass before marking complete
cd testudo-exchange && cargo clippy --all-targets && cargo test
cd testudo-extension && bun run build
```

### Edge Cases
- (list edge cases that must be handled)

---

## 3. Verification Checklist

- [ ] All new code has test coverage
- [ ] `cargo clippy --all-targets` passes with no warnings
- [ ] `cargo test` passes (all existing + new tests)
- [ ] `bun run build` passes (if extension/web changes)
- [ ] No `unwrap()` in production code
- [ ] No `f64` for financial math (use `rust_decimal`)
- [ ] Manual verification: (describe what to check visually/functionally if applicable)

---

## 4. Termination Criteria

This task is COMPLETE when ALL of the following are true:

- [ ] All functional requirements above are checked off
- [ ] All tests in section 2 pass
- [ ] All verification checks in section 3 pass
- [ ] No regressions in existing test suite
- [ ] Changes are committed with descriptive message

This task is NOT COMPLETE if:
- Any test fails
- Implementation is partial
- Verification commands produce warnings or errors
- Edge cases from section 2 are unhandled
