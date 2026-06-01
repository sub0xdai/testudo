# Handoff — Testudo CLI Harness (CLI-07 + CLI-08 complete, CI/release live)

**Date:** 2026-06-01 16:44
**Project:** testudo (testudo-cli crate)
**Next focus:** Test the end-to-end install flow; iterate on UX; consider repo split or landing page

---

## Summary

Built CLI-07 (onboarding, multi-provider LLM, strategy creation) and CLI-08 (command palette, autocomplete, settings screen) on top of the already-complete CLI-01 through CLI-06. Set up CI/CD with GitHub Actions that builds Linux, macOS, and Windows binaries on tagged releases. Deployed `install.sh` and `install.ps1` to `api.testudo.vip`. All 20 test suites pass with zero clippy warnings.

---

## Current State

| What | Status |
|------|--------|
| CLI-01 through CLI-06 | ✅ Complete (165 tests) |
| CLI-07 Onboarding + Multi-Provider LLM | ✅ Complete |
| CLI-08 Command Palette + Settings | ✅ Complete |
| `install.sh` (Linux/macOS) | ✅ Deployed to api.testudo.vip |
| `install.ps1` (Windows) | ✅ Deployed to api.testudo.vip |
| GitHub Actions CI | ✅ 4 platforms (linux x86_64, macOS x86_64/arm64, windows x86_64) |
| Release v0.1.0 | ✅ Published |
| `testudo init` LLM config step | ✅ 11 providers + Custom |
| `testudo strategy create` | ✅ Programmatic API, CLI stub |
| `testudo dashboard` command palette | ✅ `/` opens bar, Tab autocomplete, Up/Down history |
| `testudo dashboard` settings screen | ✅ `/settings` shows masked config |
| Total tests | ~200, 0 clippy warnings |

---

## Key Decisions

- **`api.testudo.vip` for install scripts** — `testudo.vip` apex was blocked by a Cloudflare Worker (testudo-web). Install scripts live on the already-working API domain instead. Clean separation: API domain for backend + installer, Worker for web app.
- **`rustls` over `native-tls`** — avoids system OpenSSL dependency, compiles cross-platform without extra CI deps.
- **11 LLM providers via 3 protocols** — Anthropic (native), OpenAI-compatible (covers 8 providers), Gemini (native). Ollama uses OpenAI-compatible client.
- **Command palette uses `/` leader** — vim/helix-style. Tab cycles cached autocomplete matches. `:` also works.
- **Daemon is Unix-only** — Unix domain sockets. Windows gets a clean error message. `#[cfg(unix)]` guards in daemon.rs, main.rs, cmd.rs.
- **Release CI: fail-fast=false** — individual platform failures don't block other builds. Publish step runs with `if: always() && !cancelled()`.

---

## Artifacts

| Artifact | Path | Description |
|----------|------|-------------|
| CLI-07 Spec | `.specify/specs/CLI-07-onboarding-distribution/spec.md` | install.sh, multi-provider LLM, strategy create |
| CLI-07 Plan | `.specify/specs/CLI-07-onboarding-distribution/IMPLEMENTATION_PLAN.md` | 5 CPs, all complete |
| CLI-08 Spec | `.specify/specs/CLI-08-command-palette/spec.md` | command palette, autocomplete, settings |
| CLI-08 Plan | `.specify/specs/CLI-08-command-palette/IMPLEMENTATION_PLAN.md` | 4 CPs, all complete |
| CI Workflow | `.github/workflows/release.yml` | Build + publish on tag push |
| install.sh | `install.sh` (repo root) | Bash installer (Linux, macOS, Git Bash) |
| install.ps1 | `install.ps1` (repo root) | PowerShell installer (Windows native) |
| Nginx config | `testudo-ops/nginx-install.conf` | Reference for serving install scripts |
| CLI README | `testudo-cli/README.md` | User-facing docs with install instructions |
| Handoff (prev) | `HANDOFF.md` (repo root) | Previous handoff from CLI-01-06 completion |
| Release | https://github.com/sub0xdai/testudo/releases/tag/v0.1.0 | 4 platform binaries |

---

## Suggested Skills

| Skill | Relevance | Invocation |
|-------|-----------|------------|
| `vox build` | If continuing spec-driven work (CLI-09 or similar) | `/skill:vox build <spec-name>` |
| `diff-review` | Generate HTML review of all CLI-07 + CLI-08 changes | `/skill:diff-review` |
| `graphify` | Rebuild knowledge graph with new code | `/skill:graphify .` |
| `cloudflare-devops` | Fix Cloudflare DNS for apex domain or manage Workers | `/skill:cloudflare-devops` |

Also consider:
- **Read first**: `testudo-cli/README.md`, `.specify/specs/notes.md`, `install.sh`
- **Tests to run**: `cd testudo-cli && cargo test && cargo clippy --all-targets`
- **Git state**: All committed, on `master`, clean working tree

---

## Open Questions / Blockers

- **testudo.vip apex domain** — blocked by a Cloudflare Worker (testudo-web) squatting on the root. Currently using `api.testudo.vip` as workaround. Fix: detach Worker from apex, add DNS A record to droplet (170.64.236.178). Needs Cloudflare dashboard or API access.
- **Interactive `testudo strategy create`** — programmatic API works, but CLI handler just prints a template. The actual interactive stdin wizard (with $EDITOR for prompts) was deferred.
- **aarch64-linux builds** — removed from CI matrix due to cross-compilation complexity. Can be added back with an arm64 runner or emulation.
- **Live backend integration testing** — API/WS clients compile and pass mock tests but haven't been tested against a running Testudo backend.
- **Repo split for open-source** — CLI depends on `common_utils` via path dependency. For standalone repo, options: publish to crates.io, vendor types, or git dependency.

---

## Redactions

(None)
