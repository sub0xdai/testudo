# Handoff — Testudo CLI Harness (CLI-01 through CLI-06 complete)

**Date:** 2026-06-01 00:02
**Project:** testudo (testudo-cli crate)
**Next focus:** Test the harness end-to-end; commit the init UX improvements; then consider repo split for open-source launch.

---

## Summary

Built the entire `testudo` CLI trading harness across 6 specs (CLI-01 through CLI-06) — 19 checkpoints total. The binary now has: TUI dashboard, REST/WS clients, LLM-powered agent loop, strategy registry with proof-backed constraints, daemon mode, and a guided init wizard. All 165 tests pass with zero clippy warnings. The last uncommitted change improved the `testudo init` flow to default to the production URL and added context to each step.

---

## Current State

| What | Status |
|------|--------|
| CLI-01 Core TUI (clap, config, dashboard) | ✅ 24 tests |
| CLI-02 API Client (REST, WebSocket, listen/journal) | ✅ 55 tests |
| CLI-03 Agent Loop (LLM, tools, idempotency) | ✅ 85 tests |
| CLI-04 Strategy Registry (builtins, risk precheck, init) | ✅ 123 tests |
| CLI-05 Daemon Polish (daemon, attach, live panes, docs) | ✅ 149 tests |
| CLI-06 Strategy System (proof bridge, validate CLI) | ✅ 165 tests |
| Init UX improvements (production default, context prompts) | 🔄 Uncommitted |
| **Total: 165 tests, 0 clippy warnings** | |

---

## Key Decisions

- **No fork() for daemon mode** — stays foreground, writes PID/socket files. Use `nohup` or `systemd` for backgrounding. Cross-platform and simpler.
- **Init flow is terminal-prompt, not TUI** — simpler, works over SSH, upgradable later.
- **Production default URL** — changed from `http://localhost:8080` to `https://testudo.vip/api/v1` so users never have to type a URL.
- **Constraint merge rule: min() for caps, max() for floors** — always picks the most conservative bound. User can only tighten, never loosen.
- **Proof artifacts loaded from `../../testudo-proofs/Proofs/`** — graceful fallback if directory doesn't exist (runs without proof constraints).
- **Binary renamed from `tudo` to `testudo`** — original was a typo; full project name is correct.
- **Client-side types defined in `api/types.rs`** — not imported from `common-utils`. Only `AgentAlert`, `ExecutionReport`, `Candle` come from common-utils.

---

## Artifacts

| Artifact | Path | Description |
|----------|------|-------------|
| CLI-01 Spec | `.specify/specs/CLI-01-core-tui/spec.md` | Core TUI scaffold requirements |
| CLI-01 Plan | `.specify/specs/CLI-01-core-tui/IMPLEMENTATION_PLAN.md` | 3 CPs, all complete |
| CLI-02 Spec | `.specify/specs/CLI-02-api-client/spec.md` | API + WS client requirements |
| CLI-02 Plan | `.specify/specs/CLI-02-api-client/IMPLEMENTATION_PLAN.md` | 4 CPs, all complete |
| CLI-03 Spec | `.specify/specs/CLI-03-agent-loop/spec.md` | LLM + tools + loop requirements |
| CLI-03 Plan | `.specify/specs/CLI-03-agent-loop/IMPLEMENTATION_PLAN.md` | 4 CPs, all complete |
| CLI-04 Spec | `.specify/specs/CLI-04-strategy-registry/spec.md` | Strategy + risk + init requirements |
| CLI-04 Plan | `.specify/specs/CLI-04-strategy-registry/IMPLEMENTATION_PLAN.md` | 5 CPs, all complete |
| CLI-05 Spec | `.specify/specs/CLI-05-daemon-polish/spec.md` | Daemon + panes + docs |
| CLI-05 Plan | `.specify/specs/CLI-05-daemon-polish/IMPLEMENTATION_PLAN.md` | 5 CPs, all complete |
| CLI-06 Spec | `.specify/specs/CLI-06-strategy-system/spec.md` | Proof bridge requirements |
| CLI-06 Plan | `.specify/specs/CLI-06-strategy-system/IMPLEMENTATION_PLAN.md` | 3 CPs, all complete |
| Specs Index | `.specify/specs/notes.md` | All spec statuses (updated) |
| Project README | `testudo-cli/README.md` | User-facing docs (NEW) |
| Agent Guide | `AGENT_TRADING.md` | Updated with testudo-first workflow |
| Proof Artifacts | `testudo-proofs/Proofs/*.toml` | 7 Lean 4 proof artifacts loaded by CLI-06 |
| Backend Routes | `testudo-exchange/crates/router/src/routes/` | 25 route files consumed by API client |
| Common Types | `testudo-exchange/crates/common_utils/src/lib.rs` | AgentAlert, ExecutionReport, Candle exports |
| Constitution | `.specify/memory/constitution.md` | Project standards |
| Anchor Manifest | `.anchor/anchor-manifest.json` | 396 anchors across codebase |
| Builtin Strategies | `testudo-cli/strategies/builtins/*.toml` | 3 strategies: mean-reversion, momentum-breakout, funding-arb |
| Cargo config | `testudo-cli/Cargo.toml` | 20 dependencies |
| Uncommitted changes | 4 files (init UX + default URL) | `cmd.rs`, `config.rs`, 2 test files |

---

## Suggested Skills

| Skill | Relevance | Invocation |
|-------|-----------|------------|
| `vox build` | If continuing spec-driven work (there's a planned but not-yet-existent CLI-07 or similar) | `/skill:vox build <spec-name>` |
| `diff-review` | To generate a visual HTML review of all the work done across the 19 checkpoints | `/skill:diff-review` |
| `graphify` | To build a knowledge graph of the entire project for onboarding | `/skill:graphify .` |

Also consider:
- **Read first**: `testudo-cli/README.md` for the user-facing docs, then `testudo-cli/src/main.rs` for the entry point
- **Tests to run**: `cd testudo-cli && cargo test && cargo clippy --all-targets`
- **Git state**: 4 files uncommitted (init UX improvements). On `master`, ahead of origin by several commits.

---

## Open Questions / Blockers

- **Repo split for open-source launch**: The CLI depends on `common_utils` via path dependency. For a standalone repo, either publish `common_utils` to crates.io, vendor the 3 shared types, or use a git dependency. The user discussed launching the backend first, then the CLI the next day.
- **`testudo init` should be tested interactively** — the init flow was tested with piped input but never with a real human going through the steps.
- **Live backend integration**: None of the API/WS commands have been tested against a running Testudo backend. They compile and have mock tests, but real end-to-end hasn't happened.
- **(None blocked)**

---

## Redactions

(None)
