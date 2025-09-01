# TDD Guard Custom Instructions for Testudo

**Mission:** To enforce a rigorous, systematic development process that guarantees the mathematical precision and reliability of the Testudo trading platform. Every change must be validated through a strict, test-first methodology.

---

## Core Philosophy: The Red-Green-Refactor Cycle

All development must follow the Red-Green-Refactor sequence without exception. Each phase has strict entry and exit criteria.

---

### **Rule 1: The Strict TDD Sequence (Global)**

This rule applies to all code changes in the repository.

#### **Phase 1: RED (Write a Failing Test)**
-   **ENTRY CRITERIA:** You are starting a new task or adding functionality.
-   **ACTION:** You **MUST** first write a single, focused test that describes a piece of the desired functionality.
-   **VALIDATION:** You **MUST** run the test suite and prove that this new test fails for the expected reason.
-   **RESTRICTION:** You **MUST NOT** write any implementation code during this phase.

#### **Phase 2: GREEN (Minimal Implementation)**
-   **ENTRY CRITERIA:** You have a single, clearly failing test.
-   **ACTION:** You **MUST** write the absolute minimum amount of implementation code required to make that single test pass.
-   **VALIDATION:** All tests in the suite now pass.
-   **RESTRICTION:** You **MUST NOT** implement any functionality not explicitly required by the current failing test. Do not add helper functions, abstractions, or optimizations at this stage.

#### **Phase 3: REFACTOR (Improve the Code)**
-   **ENTRY CRITERIA:** The entire test suite is green.
-   **ACTION:** You **MUST** now refactor the code you just wrote to improve its design, clarity, and efficiency. This is the time to run `cargo clippy` and address its suggestions.
-   **VALIDATION:** The entire test suite must remain green after every refactoring change.
-   **RESTRICTION:** You **MUST NOT** add any new functionality during this phase. Refactoring improves existing code; it does not add features.

---

### **Rule 2: Formal Verification for Financial Crates (Context-Aware)**

This is a non-negotiable rule that overrides general testing requirements in high-stakes areas.

-   **CONDITION:** The file being modified is located within `crates/disciplina/` or `crates/prudentia/`.
-   **REQUIREMENT:** The failing test written in the RED phase **MUST** be a property-based test using the `proptest!` macro. The test must verify the mathematical properties and invariants of the financial logic, not just a single set of example values.
-   **REJECTION:** A standard `#[test]` unit test with fixed inputs is **insufficient** for these crates and will be blocked.

---

### **Rule 3: Integration & End-to-End Testing Mandate (Context-Aware)**

This rule ensures that components are tested as a cohesive system, not just in isolation.

-   **CONDITION:** The file being modified is a high-level component within `crates/formatio/`, `src/`, or the frontend at `web/`.
-   **REQUIREMENT:** Tests for these areas **MUST** validate a complete workflow.
    -   For the backend (`formatio`, `src`), this means an integration test that covers the full OODA loop.
    -   For the frontend (`web/`), this means a user-centric component test (React Testing Library) or an end-to-end test (Playwright) that simulates a full user journey (e.g., drag-trade-execute).
-   **REJECTION:** Simple unit tests for complex, integrated components are insufficient and will be blocked in favor of workflow-oriented tests.
