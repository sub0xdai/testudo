# Specification: Command Palette — TUI Slash Commands + Settings Screen

**Spec ID:** CLI-08-command-palette
**Date:** 2026-06-01
**Status:** Draft
**Class:** Feature / TUI
**Priority:** P0 — a command-driven UI is the standard for modern terminal tools (lazygit, helix, vim); F-keys alone are insufficient
**Depends on:** CLI-01 through CLI-07 (all complete)
**Series:** CLI-08 (Command Palette)

---

## Problem Statement

The testudo TUI currently switches screens via F-keys (F1-F4) and quits via `q`/`Esc`. This is functional but primitive. Modern terminal tools — lazygit, helix, k9s, vim — all use a **command palette** accessed by a leader key (`:` or `/`) that opens a text input. Users type commands, autocomplete narrows the list, and Enter executes.

Without a command palette:
- Users must memorize F-key mappings (non-discoverable)
- No way to search/filter — `/strategies` would be faster than F3
- No extensibility — can't add new commands without new keybindings
- No `/settings` screen to view/edit config from within the TUI
- Feels amateurish compared to peer tools

---

## User Stories

- **As a TUI user**, I press `/` and a command input bar appears at the bottom. I type `str` and Tab autocompletes to `/strategies`. Enter opens the strategies screen.
- **As a vim user**, I press `:` and type `q` then Enter to quit — the workflow I've used for 20 years.
- **As a trader**, I press `/settings` and see my current config (provider, model, risk limits, API URL) in a read-only screen without leaving the TUI.
- **As a power user**, I press Up arrow in the command bar and my previous commands cycle through history.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Pressing `/` or `:` opens a command input bar at the bottom of the TUI, above the status bar. | High | TUI |
| FR-2 | Typing filters matching commands in real-time. Tab cycles through matches. Enter executes. | High | TUI |
| FR-3 | Supported commands: `/dashboard`, `/journal`, `/strategies`, `/logs`, `/help`, `/settings`, `/quit`, `/q`. | High | Commands |
| FR-4 | `/settings` opens a new `Screen::Settings` that displays current config values: provider, model, base URL, API key (masked), agent key (masked), risk limits. | High | Settings |
| FR-5 | Esc or Ctrl+C closes the command bar without executing. | Medium | TUI |
| FR-6 | Command history: Up/Down arrows cycle through the last 20 commands. | Medium | TUI |
| FR-7 | Command bar shows a hint: `Type /command or :command (Tab to complete, Esc to cancel)`. | Low | TUI |
| FR-8 | Existing F-key shortcuts still work alongside commands (no regression). | High | TUI |
| FR-9 | Invalid commands show a brief error flash in the command bar: `Unknown command: /xyz`. | Low | TUI |

---

## Technical Implementation

### Architecture

```
Key press flow:

  KeyPress('/')  ──▶  Enter command mode
       │
       ▼
  command_input = "/"
  command_mode = true
       │
       ▼
  KeyPress(char) ──▶  Append to command_input, filter autocomplete
  KeyPress(Tab)  ──▶  Cycle autocomplete match
  KeyPress(Enter)──▶  Parse + execute command, exit command mode
  KeyPress(Esc)  ──▶  Exit command mode, clear input
  KeyPress(Up)   ──▶  Previous history entry
  KeyPress(Down) ──▶  Next history entry
```

### State additions

```rust
// In AppState — NEW fields
pub struct AppState {
    // ... existing fields ...
    pub command_mode: bool,
    pub command_input: String,
    pub command_history: Vec<String>,
    pub command_history_idx: Option<usize>,
    pub command_error: Option<String>,  // flash message
}
```

### Supported commands

```rust
// In commands.rs — NEW file
pub enum TuiCommand {
    Dashboard,
    Journal,
    Strategies,
    Logs,
    Help,
    Settings,
    Quit,
}

impl TuiCommand {
    pub fn from_input(input: &str) -> Option<Self> {
        match input.trim() {
            "/dashboard" | ":dashboard" => Some(Self::Dashboard),
            "/journal" | ":journal" => Some(Self::Journal),
            "/strategies" | ":strategies" => Some(Self::Strategies),
            "/logs" | ":logs" => Some(Self::Logs),
            "/help" | ":help" => Some(Self::Help),
            "/settings" | ":settings" => Some(Self::Settings),
            "/quit" | "/q" | ":quit" | ":q" => Some(Self::Quit),
            _ => None,
        }
    }

    /// All available commands for autocomplete.
    pub fn all() -> &'static [&'static str] {
        &[
            "/dashboard", "/journal", "/strategies", "/logs",
            "/help", "/settings", "/quit",
        ]
    }
}
```

### Command bar rendering

The command bar sits between the main content area and the status bar:

```
┌─────────────────────────────────────────────┐
│                                             │
│              Main content area               │
│              (dashboard / screen)            │
│                                             │
├─────────────────────────────────────────────┤
│ /str_                                       │  ← Command bar (only when command_mode)
│ Type /command (Tab complete, Esc cancel)     │  ← Hint text
├─────────────────────────────────────────────┤
│ v0.1.0  SHADOW  ETH:$—  uptime 00:42  F1-4 │  ← Status bar (always)
└─────────────────────────────────────────────┘
```

When `command_mode` is false, the command bar area is not rendered at all (zero height). When true, it takes 2 rows.

### Autocomplete

As the user types, filter `TuiCommand::all()` by prefix match. Tab cycles:

```rust
fn autocomplete(input: &str) -> Vec<&str> {
    let all = TuiCommand::all();
    if input.is_empty() || input == "/" || input == ":" {
        return all.to_vec();
    }
    all.iter()
        .filter(|cmd| cmd.starts_with(input))
        .copied()
        .collect()
}
```

The rendered input shows the first match in dim text after the cursor.

### Settings screen

A new `Screen::Settings` variant renders a read-only config view:

```
┌─────────────────────────────────────────────┐
│  Settings                                    │
│                                              │
│  Backend                                        │
│  ───────                                        │
│  Base URL:     https://testudo.vip/api/v1       │
│  Agent Key:    testudo_sk_...abc123             │
│  WebSocket:    ws://localhost:8081              │
│                                              │
│  LLM                                           │
│  ───                                           │
│  Provider:     deepseek                         │
│  Model:        deepseek-chat                    │
│  API Key:      sk-****abcd                     │
│                                              │
│  Risk Limits                                  │
│  ───────────                                  │
│  Max Leverage: 5×                              │
│  Risk/Trade:   2.0%                            │
│  Max Drawdown: 20.0%                           │
│                                              │
│  Agent                                        │
│  ─────                                        │
│  Loop Interval: 60s                            │
│  Shadow Mode:   true                           │
│                                              │
│  Press Esc or q to return                      │
└─────────────────────────────────────────────┘
```

Reads from `Config::load()`. API keys are masked (first 4 chars + last 4). Config is loaded fresh on each `/settings` invocation (no stale cache).

### Message types added

```rust
pub enum Message {
    // ... existing ...
    EnterCommandMode(char),     // '/' or ':'
    CommandInput(char),         // character typed in command mode
    CommandBackspace,
    CommandTab,                 // autocomplete cycle
    CommandExecute,             // Enter pressed
    CommandCancel,              // Esc pressed
    CommandHistoryUp,
    CommandHistoryDown,
    CommandError(String),       // flash error
    ClearCommandError,
}
```

### Files

| File | Action | Purpose |
|------|--------|---------|
| `testudo-cli/src/commands.rs` | **NEW** | `TuiCommand` enum, parser, autocomplete |
| `testudo-cli/src/msg.rs` | **MODIFY** | Add command-related message variants |
| `testudo-cli/src/model/state.rs` | **MODIFY** | Add `command_mode`, `command_input`, `command_history`, `Screen::Settings` |
| `testudo-cli/src/update.rs` | **MODIFY** | Handle command messages, dispatch `TuiCommand` |
| `testudo-cli/src/view/dashboard.rs` | **MODIFY** | Render command bar when active, `Screen::Settings` |
| `testudo-cli/src/view/command_bar.rs` | **NEW** | Command bar widget |
| `testudo-cli/src/view/settings.rs` | **NEW** | Settings screen widget |
| `testudo-cli/src/lib.rs` | **MODIFY** | Export new modules |

### Vertical Checkpoints

| CP | Scope | Validates |
|----|-------|-----------|
| CP-1 | Command mode toggle + input rendering — pressing `/` shows bar, typing displays, Esc cancels | Visual: bar appears, text renders, Esc clears |
| CP-2 | Command parser + execution — Enter on `/strategies` switches screen, `/quit` exits | Tests: `TuiCommand::from_input` parses all variants |
| CP-3 | Autocomplete + history — Tab cycles matches, Up/Down navigates history | Tests: autocomplete returns filtered list, history bounded at 20 |
| CP-4 | Settings screen — new `Screen::Settings`, reads Config, renders masked view | Tests: settings screen renders without panic, keys masked |

---

## Acceptance Criteria

- [ ] Pressing `/` opens command bar with `/` pre-filled
- [ ] Pressing `:` opens command bar with `:` pre-filled
- [ ] Typing characters appends to input, renders correctly
- [ ] Tab autocompletes to first matching command
- [ ] Multiple Tab presses cycle through matches
- [ ] Enter on `/strategies` switches to Strategies screen
- [ ] Enter on `/quit` or `/q` exits the TUI
- [ ] Enter on `/settings` opens settings screen with masked keys
- [ ] Esc closes command bar without executing
- [ ] Up/Down arrows cycle through command history
- [ ] F1-F4 still work when not in command mode
- [ ] Invalid command shows error flash
- [ ] `cargo clippy --all-targets && cargo test` passes

---

## Risks

1. **Ratatui text input is manual** — ratatui has no native text input widget. Mitigation: we track cursor position and input buffer manually. This is well-understood pattern used by lazygit, tui-rs examples, etc.
2. **Command bar steals focus** — during command mode, F-keys should still work or be explicitly disabled. Mitigation: F-keys are ignored in command mode until Esc/Cancel. This is vim's behavior.
3. **Settings screen is read-only for now** — editing config from TUI adds complexity (TOMl serialization, validation). Mitigation: phase 1 is read-only display. Editing deferred to CLI-09.

---

## Completion Signal

1. `/` or `:` opens command bar with autocomplete
2. All 7 commands execute correctly
3. `/settings` displays readable config
4. History persists across commands within a session
5. Zero regressions on F-key shortcuts
6. `cargo clippy --all-targets && cargo test` passes
7. Code committed
