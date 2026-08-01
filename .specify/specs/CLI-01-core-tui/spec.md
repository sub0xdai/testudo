# Specification: Core Crate + TUI Scaffold

**Spec ID:** CLI-01-core-tui
**Date:** 2026-05-31
**Status:** Draft
**Class:** Feature / Application
**Priority:** P1 — foundation for the entire `tudo` harness; nothing else works without it
**Depends on:** AGENT-07-agent-api-keys (for `tudo_sk_...` config format)
**Series:** CLI-01 (Core TUI)

---

## Problem Statement

Testudo has a complete backend (signal endpoint, WebSocket alerts, journal memory, onboarding, agent keys) but no terminal-first client. Users and LLM agents interact through raw HTTP calls or the browser journal. There's no live dashboard, no keyboard-driven workflow, no single binary that ties the platform together.

This spec establishes the `tudo` crate — the single binary that will eventually run the full trading harness. It delivers the CLI scaffold, TEA-based TUI loop, config loading, credential storage, and a working (but data-empty) dashboard. Every subsequent CLI spec adds behavior to this foundation.

---

## User Stories

- **As a developer**, I want to run `tudo dashboard` and see a correctly-laid-out TUI with all panes visible, so that I know the rendering pipeline works before data flows in.
- **As a user**, I want the harness to read my API key from `~/.config/tudo/config.toml`, so that subsequent specs don't need to reinvent credential management.
- **As an n0x operator**, I want `cargo build` in `tudo/` to produce a binary I can run, so that I can verify the harness builds before any trading logic is added.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | `tudo` binary with clap CLI: `init`, `agent start/stop/pause/resume`, `dashboard`, `listen`, `journal`, `strategy list/add/show/remove`, `attach`. All subcommands print a descriptive "not yet implemented" message except `dashboard`. | High | CLI |
| FR-2 | `tudo dashboard` opens a ratatui TUI with 6 labeled panes in correct layout: positions (top-left), agent reasoning (top-right), P&L chart (mid-left), signal log (mid-right), journal summary (bottom-left), risk (bottom-right). Status bar at bottom showing version, mode, and help keys. | High | TUI |
| FR-3 | TEA event loop: `tokio::select!` over crossterm key events, 1Hz tick timer, and a message channel. Key events produce `Message::KeyPress`. Tick produces `Message::Tick`. TUI renders at key-press speed; data refresh placeholder at 1fps. | High | TUI |
| FR-4 | Config loaded from `~/.config/tudo/config.toml` at startup. Schema: `[api]` with `base_url` and `agent_key`; `[agent]` with `loop_interval_secs` and `shadow_only`; `[llm]` with `provider` and `api_key`. Missing config creates a default file with comments. | High | Config |
| FR-5 | Screen navigation: F1→Dashboard, F2→Journal, F3→Strategies, F4→Logs, `?`→Help, `q`/`Esc`→Quit. Screen switching updates the TUI immediately. | High | TUI |
| FR-6 | `cargo clippy && cargo test` passes in `tudo/`. | High | CI |

---

## Technical Implementation

### Architecture

Hand-rolled TEA loop (skip `tears` — use `ratatui` + `tokio::select!` directly per Risk #1 in the blueprint). The pattern:

```
main.rs
  ├─ parse CLI → Command
  ├─ load config → Config
  └─ match command:
       Dashboard → run_app(config)
       _         → println!("not yet implemented: {command}")

run_app:
  ├─ init terminal (crossterm)
  ├─ spawn tick timer (tokio::interval 1s → Message::Tick)
  ├─ spawn key reader (crossterm event stream → Message::KeyPress)
  └─ loop {
       tokio::select! {
         msg = rx.recv() → update(model, msg) → view(terminal, &model)
         _ = tick.tick() → update(model, Tick) → view(terminal, &model)
       }
     }
```

### Crate Structure (this spec only)

```
tudo/
├── Cargo.toml
├── src/
│   ├── main.rs              // clap CLI + app entry
│   ├── app.rs               // TEA loop: run_app()
│   ├── model/
│   │   ├── mod.rs
│   │   └── state.rs         // AppState, Screen enum, StatusBar
│   ├── msg.rs               // Message enum (KeyPress, Tick, Resize, SwitchScreen, Quit)
│   ├── update.rs            // Pure update function: (model, msg) → model
│   ├── view/
│   │   ├── mod.rs
│   │   ├── dashboard.rs     // Main dashboard layout (6 panes)
│   │   ├── help.rs          // Help screen (keybindings table)
│   │   └── status_bar.rs    // Bottom bar renderer
│   ├── config.rs            // Config struct + load/save + XDG path resolution
│   └── auth.rs              // Credential storage scaffold (reads agent_key from config)
└── tests/
    └── config_tests.rs
```

### Model

```rust
// src/model/state.rs

pub struct AppState {
    pub screen: Screen,
    pub status: StatusBar,
    pub theme: Theme,
    pub error: Option<String>,
}

pub enum Screen {
    Dashboard,
    Journal,
    Strategies,
    Logs,
    Help,
}

pub struct StatusBar {
    pub version: String,
    pub mode: String,          // "SHADOW" or "LIVE" (hardcoded "SHADOW" for now)
    pub last_ticker: String,   // Placeholder: "ETH: $—"
    pub uptime: String,        // Placeholder: "0h 0m"
}
```

### Theme

```rust
// src/theme.rs

/// Color palette for the entire TUI.
/// All panes and widgets reference this struct — no hardcoded colors in view code.
#[derive(Debug, Clone)]
pub struct Theme {
    // ── Canvas ──
    pub bg: Color,
    pub fg: Color,
    pub dim_fg: Color,

    // ── Borders & separators ──
    pub border: Color,
    pub border_focused: Color,

    // ── Semantic colors ──
    pub accent: Color,         // Primary highlight (headings, selected items)
    pub success: Color,        // Profit, filled orders, health
    pub danger: Color,         // Loss, drawdown warnings, rejected signals
    pub warning: Color,        // Alerts, medium severity
    pub info: Color,           // Neutral information
    pub muted: Color,          // Secondary text, disabled items

    // ── Pane-specific ──
    pub positions_header: Color,
    pub positions_long: Color,
    pub positions_short: Color,
    pub pnl_positive: Color,
    pub pnl_negative: Color,
    pub signal_filled: Color,
    pub signal_rejected: Color,
    pub signal_pending: Color,
    pub risk_gauge_fill: Color,
    pub risk_gauge_bg: Color,
    pub status_bar_bg: Color,
    pub status_bar_fg: Color,

    // ── TUI chrome ──
    pub help_key: Color,       // Keybinding hints
    pub help_desc: Color,      // Description text
    pub input_cursor: Color,   // Text input cursor
}

impl Theme {
    /// Vanilla Amoled — true black background, desaturated pastel accents.
    /// Background: pure AMOLED black (#000000). Text: light gray.
    /// Accent: faint blue. Semantic: muted green/red/yellow.
    /// Matches the pi.dev `vanilla-amoled` theme for visual consistency.
    pub fn vanilla_amoled() -> Self {
        Self {
            bg: Color::Rgb(0, 0, 0),                // #000000 — AMOLED black
            fg: Color::Rgb(187, 187, 187),           // #BBBBBB — light gray
            dim_fg: Color::Rgb(102, 102, 102),       // #666666

            border: Color::Rgb(74, 74, 74),          // #4A4A4A — subtle border
            border_focused: Color::Rgb(102, 102, 102), // #666666

            accent: Color::Rgb(138, 154, 184),       // #8A9AB8 — faint blue
            success: Color::Rgb(129, 168, 134),      // #81A886 — faint green
            danger: Color::Rgb(179, 128, 128),       // #B38080 — faint red
            warning: Color::Rgb(179, 168, 112),      // #B3A870 — faint yellow
            info: Color::Rgb(122, 154, 154),         // #7A9A9A — faint cyan
            muted: Color::Rgb(153, 153, 153),        // #999999

            positions_header: Color::Rgb(138, 154, 184),
            positions_long: Color::Rgb(129, 168, 134),
            positions_short: Color::Rgb(179, 128, 128),
            pnl_positive: Color::Rgb(129, 168, 134),
            pnl_negative: Color::Rgb(179, 128, 128),
            signal_filled: Color::Rgb(129, 168, 134),
            signal_rejected: Color::Rgb(179, 128, 128),
            signal_pending: Color::Rgb(179, 168, 112),
            risk_gauge_fill: Color::Rgb(129, 168, 134),
            risk_gauge_bg: Color::Rgb(51, 51, 51),   // #333333 — surface4
            status_bar_bg: Color::Rgb(8, 8, 8),      // #080808 — surface0
            status_bar_fg: Color::Rgb(153, 153, 153),

            help_key: Color::Rgb(138, 154, 184),
            help_desc: Color::Rgb(187, 187, 187),
            input_cursor: Color::Rgb(138, 154, 184),
        }
    }

    /// Load theme from config name. Currently only "vanilla-amoled" exists.
    /// Future: "kanso-ink", "tokyo-night", "nord", "solarized-dark".
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "vanilla-amoled" => Self::vanilla_amoled(),
            other => {
                tracing::warn!("Unknown theme '{}', falling back to vanilla-amoled", other);
                Self::vanilla_amoled()
            }
        }
    }
}
```

All view functions receive `&Theme` and use it for every `Style`. Example:
```rust
// In any pane renderer:
let block = Block::default()
    .title(" Positions ")
    .borders(Borders::ALL)
    .border_style(Style::default().fg(theme.border))
    .title_style(Style::default().fg(theme.accent));

let pnl = Span::styled(
    format!("${:+.2}", pos.unrealized_pnl),
    Style::default().fg(if pos.unrealized_pnl >= 0.0 { theme.pnl_positive } else { theme.pnl_negative }),
);
```

```rust
// src/msg.rs

pub enum Message {
    KeyPress(KeyEvent),
    Resize(u16, u16),
    Tick,
    SwitchScreen(Screen),
    ShowHelp,
    Quit,
    Error(String),
    ClearError,
}
```

### Config Schema

```toml
# ~/.config/tudo/config.toml (auto-generated default)

[ui]
theme = "vanilla-amoled"         # "vanilla-amoled" only for now; future: "kanso-ink", "tokyo-night", "nord"

[api]
base_url = "http://localhost:8080/api/v1"
agent_key = ""                  # tudo_sk_... from AGENT-07

[agent]
loop_interval_secs = 60
shadow_only = true

[llm]
provider = "anthropic"
api_key = ""
model = "claude-sonnet-4-20250514"
```

### Dependencies

```toml
[dependencies]
# TUI
ratatui = "0.29"
crossterm = "0.28"

# Async
tokio = { version = "1", features = ["full"] }

# CLI
clap = { version = "4", features = ["derive"] }

# Config
toml = "0.8"
serde = { version = "1", features = ["derive"] }
directories = "5"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### Dependencies NOT in this spec
- `reqwest`, `tokio-tungstenite` → CLI-02
- `uuid`, LLM crates → CLI-03
- `common-utils` (backend types) → CLI-02
- `sha2`, `hex` → not needed (agent key is opaque string, no crypto needed client-side)

---

## Checkpoints

### CP-1: Crate scaffold + clap CLI
- **Touches**: `tudo/Cargo.toml` (NEW), `tudo/src/main.rs` (NEW)
- **Tasks**:
  1. Create `tudo/Cargo.toml` with dependencies above (ratatui, crossterm, tokio, clap, toml, serde, directories, tracing).
  2. Create `tudo/src/main.rs` with clap derive CLI: `Command` enum with `Init`, `Agent { action: AgentAction }`, `Dashboard`, `Listen`, `Journal`, `Strategy { action: StrategyAction }`, `Attach`.
  3. All non-Dashboard commands print `println!("not yet implemented: {command:?}")` and exit 0.
  4. Verify: `cargo build` in `tudo/` succeeds. `tudo dashboard` prints "not yet implemented" (no TUI yet). `tudo agent start`, `tudo listen`, etc. all print and exit cleanly.
- **Verification**: `cargo build -p tudo` exits 0. `cargo run -- dashboard` prints stub message. `cargo clippy -p tudo --all-targets` passes.

### CP-2: Config loading
- **Touches**: `tudo/src/config.rs` (NEW), `tudo/src/main.rs`
- **Tasks**:
  1. Define `Config` struct with `#[derive(Debug, Deserialize, Serialize)]` — fields: `ui: UiConfig` (theme name), `api: ApiConfig`, `agent: AgentConfig`, `llm: LlmConfig`. Use `directories::ProjectDirs` for XDG paths. On load, resolve theme via `Theme::from_name(config.ui.theme)`.
  2. `Config::load()` — reads `~/.config/tudo/config.toml`. If file doesn't exist, creates it with defaults and comments, then returns defaults. If parse fails, prints error and location, exits 1.
  3. Unit test: `Config::default()` has correct base_url, interval=60, shadow_only=true.
  4. Unit test: round-trip serialize → deserialize preserves values.
  5. Wire into `main.rs`: load config before matching on command. For Dashboard, pass config to `run_app()`. For other commands, just confirm config loaded.
- **Verification**: `cargo test -p tudo` passes config tests. Run binary with no config → `~/.config/tudo/config.toml` created. Run again → reads existing.

### CP-3: TUI loop + dashboard layout
- **Touches**: `tudo/src/app.rs` (NEW), `tudo/src/model/state.rs` (NEW), `tudo/src/model/mod.rs` (NEW), `tudo/src/msg.rs` (NEW), `tudo/src/update.rs` (NEW), `tudo/src/theme.rs` (NEW), `tudo/src/view/mod.rs` (NEW), `tudo/src/view/dashboard.rs` (NEW), `tudo/src/view/status_bar.rs` (NEW), `tudo/src/main.rs`
- **Tasks**:
  1. Implement `run_app(config: Config)` in `app.rs`: init crossterm terminal, enter raw mode, spawn tick timer (1s interval → `Message::Tick`), spawn key reader (crossterm `EventStream` → `Message::KeyPress`), run TEA loop.
  2. Implement `update(model, msg)` in `update.rs`: KeyPress `q`/`Esc` → return `Quit`. F1-F4 → `SwitchScreen(...)`. Tick → increment uptime counter. Resize → update terminal size.
  3. Implement `Theme` in `theme.rs`: `Theme::blackboard()` constructor, `Theme::from_name()` dispatcher. Store in `AppState.theme` (loaded from config `ui.theme` field, defaulting to `"blackboard"`).
  4. Implement `view(f, &model)` in `view/dashboard.rs`: render 6 empty labeled panes using `ratatui::layout::Layout` (3-row × 2-col split). Every `Style` uses `model.theme` fields — no hardcoded colors:
     - Top-left: "Positions" (bordered block, `theme.border` / `theme.accent` title)
     - Top-right: "Agent Reasoning" (bordered block)
     - Mid-left: "P&L Chart" (bordered block)
     - Mid-right: "Signal Log" (bordered block)
     - Bottom-left: "Journal Summary" (bordered block)
     - Bottom-right: "Risk" (bordered block)
     - Status bar at bottom: `theme.status_bar_bg` background, `theme.status_bar_fg` text. Content: `tudo v0.1.0 | SHADOW | ETH: $— | 0h 0m | F1 Dash F2 Jnl F3 Strats F4 Logs q Quit`
  5. Wire `tudo dashboard` CLI command to call `run_app()`.
- **Verification**: `cargo run -- dashboard` opens TUI. All 6 panes visible with borders and labels. Status bar shows version. Press `q` → exits cleanly (terminal restored). F1-F4 switch between screens (show placeholder text per screen). `cargo clippy -p tudo --all-targets && cargo test -p tudo` passes.

---

## Acceptance Criteria

- [ ] `tudo` binary builds with `cargo build`
- [ ] All stub commands print "not yet implemented" and exit 0
- [ ] `tudo dashboard` opens TUI with 6 labeled panes in correct 3×2 layout
- [ ] Status bar renders version, mode, and keybinding hints
- [ ] F1-F4 switch screens (Dashboard, Journal, Strategies, Logs — all placeholder screens)
- [ ] `q` and `Esc` exit cleanly, terminal restored
- [ ] Config auto-created at `~/.config/tudo/config.toml` with defaults on first run
- [ ] Config reloaded on subsequent runs preserving user edits
- [ ] `cargo clippy --all-targets && cargo test` passes in `tudo/`
- [ ] TUI never blocks — keypresses respond instantly (60fps feel), tick updates at 1fps

---

## Risks

1. **`tears` immaturity** — The blueprint suggested `tears` as a TEA framework. We skip it entirely. A hand-rolled `tokio::select!` loop with `ratatui` is ~150 lines and avoids an external dependency that may have bugs or lag in updates.
2. **XDG path resolution** — The `directories` crate handles Linux/macOS. If deploying to a minimal container, `~/.config` may not exist. Mitigation: `Config::load()` gracefully creates the directory tree.
3. **Terminal restore on panic** — If the TUI loop panics, the terminal stays in raw mode (no echo, broken display). Mitigation: install a panic hook in `main.rs` that calls `crossterm::terminal::disable_raw_mode()` before printing the panic message.

---

## Completion Signal

This spec is complete when:
1. `tudo` binary exists with correct CLI structure
2. TUI dashboard renders with 6 panes + status bar
3. Screen navigation works (F1-F4, q, Esc)
4. Config loads from `~/.config/tudo/config.toml` with auto-creation
5. `cargo clippy --all-targets && cargo test` passes in `tudo/`
6. Code committed to master under `tudo/`

---

## Next Spec

**CLI-02-api-client** — Adds REST API client, WebSocket client, `tudo listen`, and `tudo journal` commands. Depends on the config and TUI loop from this spec.
