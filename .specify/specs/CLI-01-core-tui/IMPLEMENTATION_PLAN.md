# CLI-01-core-tui — Implementation Plan

## Current State Summary

The `testudo-cli/` crate (binary name `tudo`) has a full module tree declared in `lib.rs` matching the eventual harness structure — 50+ files across models, views, tools, LLM providers, API clients, WebSocket, strategies, risk, and daemon. Every single file is an empty stub containing only an `@anchor` tag and a doc comment. No implementation code exists. The crate does not build: `Cargo.toml` includes dependencies for later specs (`reqwest`, `tokio-tungstenite`, `ring`, `sha2`, `hex`, `uuid`, `chrono`, `rust_decimal`, `thiserror`, `common-utils`) and the `common-utils` path dependency is broken (`common-utils` hyphen vs `common_utils` underscore). There are no tests — `tests/fixtures/` and `tests/integration/` contain only `.gitkeep` files.

The spec calls for `tudo/` as the crate directory; the codebase uses `testudo-cli/`. All file paths are mapped accordingly. The spec also adds `theme.rs` which does not exist in the current module tree and must be added to `lib.rs`.

### Gap Summary

| Requirement | Status | Detail |
|---|---|---|
| FR-1: clap CLI (7 subcommands, 6 stubs) | ❌ None | `main.rs` is a placeholder println |
| FR-2: TUI dashboard (6 panes + status bar) | ❌ None | All 8 view files are empty stubs |
| FR-3: TEA event loop (tokio::select!) | ❌ None | `app.rs`, `msg.rs`, `update.rs` all stubs |
| FR-4: Config loading (XDG + auto-create) | ❌ None | `config.rs` is empty stub |
| FR-5: Screen navigation (F1-F4, q, Esc) | ❌ None | `model/state.rs` empty stub |
| FR-6: cargo clippy + cargo test | ❌ Fail | Crate doesn't build |
| Theme system | ❌ Missing | No `theme.rs`, not in module tree |
| Cargo.toml | ❌ Broken | Extra deps, broken `common-utils` path |

---

## Checkpoints

### CP-1: Crate scaffold + clap CLI ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `testudo-cli/Cargo.toml`, `testudo-cli/src/main.rs`, `testudo-cli/src/lib.rs`
- **Tasks**:
  1. Slim `Cargo.toml` to CLI-01 dependencies only (ratatui, crossterm, tokio, clap, toml, serde, directories, tracing, tracing-subscriber). Remove: reqwest, tokio-tungstenite, futures-util, serde_json, ring, sha2, hex, tracing-appender, uuid, chrono, rust_decimal, thiserror, common-utils.
  2. Implement `main.rs`: clap derive `Command` enum — `Init`, `Agent { action: AgentAction }` (Start/Stop/Pause/Resume), `Dashboard`, `Listen`, `Journal`, `Strategy { action: StrategyAction }` (List/Add/Show/Remove), `Attach`. All non-Dashboard variants print `"not yet implemented: {command:?}"` and exit 0. Dashboard prints `"Dashboard TUI not yet implemented — CP-3."`.
  3. Verify: `cargo build` in `testudo-cli/` succeeds. `cargo run -- dashboard` prints stub message. `cargo run -- agent start` prints stub. `cargo run -- init` prints stub. `cargo clippy --all-targets` passes.
- **Verification**: `cd testudo-cli && cargo build && cargo clippy --all-targets` exits 0. Manual: `cargo run -- dashboard` prints `Dashboard TUI not yet implemented — CP-3.`
- **Commit message**: `feat: scaffold clap CLI with 7 subcommands and stub handlers`

### CP-2: Config loading ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `testudo-cli/src/config.rs` (rewrite), `testudo-cli/src/main.rs` (wire config), `testudo-cli/tests/config_tests.rs` (NEW)
- **Tasks**:
  1. Implement `Config` struct: `#[derive(Debug, Deserialize, Serialize)]` with `ui: UiConfig` (theme), `api: ApiConfig` (base_url, agent_key), `agent: AgentConfig` (loop_interval_secs, shadow_only), `llm: LlmConfig` (provider, api_key, model). All fields match spec's TOML schema exactly.
  2. `Config::load()` — uses `directories::ProjectDirs::from("com", "testudo", "tudo")`. Reads `config.toml` from config dir. If file doesn't exist, creates it with defaults and TOML comments, then returns defaults. If parse fails, prints error with file path and exits 1. Creates parent directories as needed.
  3. `Config::default()` — returns spec defaults: base_url=`http://localhost:8080/api/v1`, loop_interval_secs=60, shadow_only=true, provider=`"anthropic"`, model=`"claude-sonnet-4-20250514"`, theme=`"vanilla-amoled"`.
  4. Unit test: `default()` produces correct values.
  5. Unit test: round-trip serialize → deserialize preserves all fields.
  6. Wire into `main.rs`: load config at startup. Pass to Dashboard handler (for CP-3). Print config summary for all stubs: `"Config loaded: {base_url}"`.
- **Verification**: `cd testudo-cli && cargo test` passes both tests. Manual: run binary with no config → `~/.config/tudo/config.toml` created with defaults. Edit file, run again → edits preserved.
- **Commit message**: `feat: config loading from XDG path with auto-creation`

### CP-3: TUI loop + dashboard layout ✅
Completed 2025-05-31 by /skill:vox build

- **Touches**: `testudo-cli/src/theme.rs` (NEW), `testudo-cli/src/lib.rs` (add `mod theme`), `testudo-cli/src/model/state.rs`, `testudo-cli/src/msg.rs`, `testudo-cli/src/update.rs`, `testudo-cli/src/app.rs`, `testudo-cli/src/view/dashboard.rs`, `testudo-cli/src/view/status_bar.rs`, `testudo-cli/src/main.rs` (wire dashboard)
- **Tasks**:
  1. Create `theme.rs`: `Theme` struct per spec (vanilla-amoled palette with all semantic color fields). `Theme::from_name()` dispatcher. Store on `AppState.theme`.
  2. Implement `model/state.rs`: `AppState` struct (screen: Screen, theme: Theme, status: StatusBar, error: Option<String>). `Screen` enum (Dashboard, Journal, Strategies, Logs, Help). `StatusBar` struct (version, mode, last_ticker, uptime).
  3. Implement `msg.rs`: `Message` enum — `KeyPress(KeyEvent)`, `Resize(u16,u16)`, `Tick`, `SwitchScreen(Screen)`, `ShowHelp`, `Quit`, `Error(String)`, `ClearError`.
  4. Implement `update.rs`: pure `fn update(state: &mut AppState, msg: Message) -> bool` (returns true if should continue). KeyPress: `q`/`Esc` → false (quit). F1→Dashboard, F2→Journal, F3→Strategies, F4→Logs, `?`→Help. Tick → increment uptime seconds. Resize → store dimensions. Error → set error field.
  5. Implement `app.rs`: `run_app(config: Config)` — init crossterm (alternate screen + raw mode), install panic hook for terminal restore, spawn ticker (`tokio::interval 1s` → Message::Tick), spawn key reader (crossterm EventStream → Message::KeyPress), run TEA loop with `tokio::select!`. Terminal restore in Drop or explicit cleanup. Use `ratatui::Terminal` with `CrosstermBackend`.
  6. Implement `view/dashboard.rs`: `fn render(frame: &mut Frame, state: &AppState)` — 3-row × 2-col `Layout`. Each pane is a `Block::bordered()` with title. All colors from `state.theme`. Six panes: "Positions", "Agent Reasoning", "P&L Chart", "Signal Log", "Journal Summary", "Risk" — each showing placeholder text ("No data").
  7. Implement `view/status_bar.rs`: `fn render(frame: &mut Frame, state: &AppState, area: Rect)` — bottom bar with `state.theme.status_bar_bg` background. Content: `testudo v0.1.0 | SHADOW | ETH: $— | {uptime} | F1 Dash F2 Jnl F3 Strats F4 Logs q Quit`.
  8. Wire `tudo dashboard` in `main.rs` to call `run_app(config)`.
  9. Add `mod theme` to `lib.rs`.
- **Verification**: `cd testudo-cli && cargo run -- dashboard` opens TUI. All 6 panes visible with borders and labels. Status bar shows version + key hints. Press `q` → exits cleanly (terminal restored, prompt visible). F1-F4 switch between placeholder screens. `cargo clippy --all-targets && cargo test` passes.
- **Commit message**: `feat: TUI dashboard with 6 panes, status bar, TEA event loop, and screen navigation`

---

## Risks & Open Questions

1. **Crate location** — Spec says `tudo/`; codebase uses `testudo-cli/`. Resolved: use `testudo-cli/` throughout. The binary name `tudo` is preserved via `[[bin]] name = "tudo"`.
2. **`common-utils` removal** — The existing `Cargo.toml` has `common-utils = { path = "../testudo-exchange/crates/common_utils" }` which doesn't build (hyphen/underscore mismatch + not needed for CLI-01). Removed in CP-1. Will be re-added correctly in CLI-02.
3. **Extra dependencies** — `Cargo.toml` has deps for later specs. These are removed in CP-1 and will be re-introduced in their respective specs. No backward-compat concern since nothing depends on this crate yet.
4. **Theme file** — Not in current module tree. Added as `mod theme` in CP-3.
5. **Panic hook** — Terminal restore on panic is critical for UX. Install in `app.rs` `run_app()` via `std::panic::set_hook()`.
6. **Directory creation** — `Config::load()` must create `~/.config/tudo/` if absent. The `directories` crate returns the path but doesn't create it; we handle this in `Config::load()`.

---

## Assumptions (confirm with user)

- Working directory for builds is `testudo-cli/`, not workspace root. Verified: the crate is standalone (not in `testudo-exchange` workspace).
- `Theme::vanilla_amoled()` is the only theme for CLI-01. Future themes (`kanso-ink`, etc.) added later.
- Placeholder screens (Journal, Strategies, Logs) show "Not yet implemented" centered text — not empty panes.
- The binary name is `testudo` (matches `[[bin]] name = "testudo"` in Cargo.toml).
