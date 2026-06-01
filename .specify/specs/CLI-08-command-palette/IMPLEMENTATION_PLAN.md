# CLI-08-command-palette — Implementation Plan

## Current State Summary

The TUI has 5 screens (Dashboard, Journal, Strategies, Logs, Help) navigated via F1-F4 keys. There is no command input, no autocomplete, no settings screen, and no command history. The `Message` enum has 8 variants; none are command-related.

## Checkpoints

### CP-1: Command mode toggle + input rendering ✅
- **Touches**: `testudo-cli/src/msg.rs` (new variants), `testudo-cli/src/model/state.rs` (new fields), `testudo-cli/src/update.rs` (handle command messages), `testudo-cli/src/view/command_bar.rs` (NEW), `testudo-cli/src/view/dashboard.rs` (render bar)
- **Tasks**:
  1. Add `command_mode`, `command_input`, `command_history`, `command_history_idx`, `command_error` to `AppState`
  2. Add `EnterCommandMode(char)`, `CommandInput(char)`, `CommandBackspace`, `CommandCancel`, `CommandError(String)`, `ClearCommandError` to `Message`
  3. In `update()`, handle `/` and `:` keys to enter command mode; handle Esc to cancel; handle regular chars to append
  4. Create `view/command_bar.rs` — renders input buffer + hint text in 2-row area
  5. In `view/dashboard.rs`, split layout to include command bar row when `command_mode` is true
  6. Add test for command mode toggle (enter/exit state transitions)
- **Verification**: `cargo test` passes. Visual: pressing `/` shows bar, typing renders, Esc clears.
- **Commit message**: `feat: command bar with input rendering and Esc cancel`
- Completed 2026-06-01 by /skill:vox build

### CP-2: Command parser + execution ✅
- **Touches**: `testudo-cli/src/commands.rs` (NEW), `testudo-cli/src/msg.rs` (already done), `testudo-cli/src/update.rs` (dispatch commands)
- **Tasks**:
  1. Create `commands.rs` with `TuiCommand` enum + `from_input()` parser + `all()` list
  2. Add `CommandExecute`, `CommandHistoryUp`, `CommandHistoryDown` to `Message`
  3. In `update()`, handle Enter to parse + execute command (switch screen or quit)
  4. Wire existing `Message::SwitchScreen` and `Message::Quit` variants
  5. Add unit tests for parser (all 7 commands, invalid input returns None)
- **Verification**: `cargo test` passes. Parser test covers all variants. Enter on `/strategies` switches screen.
- **Commit message**: `feat: command parser with /slash command execution`
- Completed 2026-06-01 by /skill:vox build

### CP-3: Autocomplete + history ✅
- **Touches**: `testudo-cli/src/commands.rs` (autocomplete fn), `testudo-cli/src/msg.rs` (add `CommandTab`), `testudo-cli/src/update.rs` (Tab/Up/Down handlers), `testudo-cli/src/view/command_bar.rs` (render autocomplete hint), `testudo-cli/src/model/state.rs` (autocomplete tracking fields)
- **Tasks**:
  1. Add `autocomplete(input) -> Vec<String>` function with colon prefix normalization
  2. Track `autocomplete_matches` and `autocomplete_idx` in `AppState`
  3. Tab cycles matches, Up/Down navigates history (last 20)
  4. Render autocomplete hint in dim text after cursor
  5. Add unit tests for autocomplete filtering + history bounds
- **Verification**: `cargo test` passes. Tab on `/str` cycles `/strategies` ↔ `/settings`. Up restores last command.
- **Commit message**: `feat: command autocomplete with Tab cycling and history`
- Completed 2026-06-01 by /skill:vox build

### CP-4: Settings screen ✅
- **Touches**: `testudo-cli/src/view/settings.rs` (NEW), `testudo-cli/src/view/dashboard.rs` (render Settings), `testudo-cli/src/update.rs` (Settings → q to dashboard)
- **Tasks**:
  1. Create `view/settings.rs` — renders read-only config view with masked API keys
  2. Config loaded fresh via `Config::load()` on each `/settings` invocation
  3. API keys masked: show prefix + 4 chars, then mask, then last 4
  4. Wire `Screen::Settings` into `dashboard::render()`
  5. `q`/`Esc` from Settings screen returns to Dashboard (not quit)
  6. 5 unit tests for mask_key function
- **Verification**: `cargo test` passes. `/settings` renders config with masked keys. Esc returns to dashboard.
- **Commit message**: `feat: settings screen with read-only config view`
- Completed 2026-06-01 by /skill:vox build

---

## Risks

1. **Ratatui has no native input widget** — must manage cursor position manually. Mitigation: track `cursor_pos` in `AppState`.
2. **F-key conflict during command mode** — Mitigation: F-keys ignored while `command_mode` is true, except `Esc` to cancel.
3. **Settings screen stale config** — Mitigation: reload `Config::load()` each time `/settings` is invoked.

Plan ready: 4 checkpoints, ~6-8 hours total. Run `/skill:vox build CLI-08-command-palette` to start CP-1.
