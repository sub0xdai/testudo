# E.5 Implementation Prompt: Mode Toggle UI

## Your Task

Implement **E.5: Mode Toggle** - a frontend UI component that allows traders to switch between Shadow-only (paper trading) and Live execution modes.

---

## Before You Start

**Use the Ralph Loop process for this implementation:**
```
/ralph-loop:ralph-loop
```

This will guide you through iterative implementation with checkpoints.

---

## Context

Read these files first:
- `hybrid_trading.json` - PRD with requirements and acceptance criteria
- `HANDOFF.md` - Technical reference
- `.ralph/progress.md` - Implementation history

### What's Already Built

| Phase | Status | Description |
|-------|--------|-------------|
| A-D | Complete | Market data, Shadow Engine, Risk Engine, Trade Management |
| E.1 | Complete | API key storage (encrypted) |
| E.2 | Complete | Decision Loop (risk validation before execution) |
| E.3 | Complete | Binance order execution |
| E.4 | Complete | Position sync between Shadow and Binance |
| **E.5** | **YOUR TASK** | Mode toggle UI |

---

## E.5 Requirements (from PRD)

### Acceptance Criteria

```json
"E.5": {
  "mode_toggle": "UI switch: Shadow Only | Live Trading",
  "confirmation": "Live mode requires explicit confirmation dialog",
  "visual_indicator": "Clear badge showing current mode (green=shadow, red=live)"
}
```

### Implementation Details

**1. Mode Toggle Component**
- Location: `testudo-web/apps/web/src/components/`
- Toggle switch with two states: Shadow Only | Live Trading
- Default to Shadow mode on load

**2. Confirmation Dialog**
- When switching TO Live mode, show confirmation dialog
- Dialog text: "You are about to enable live trading. Real orders will be placed on Binance."
- Require explicit "Enable Live Trading" button click
- No confirmation needed when switching back to Shadow

**3. Visual Indicator**
- Persistent badge/indicator showing current mode
- Shadow mode: Green badge, text "Paper Trading" or "Shadow"
- Live mode: Red badge, text "LIVE" - highly visible
- Should be visible on all trading screens

**4. State Management**
- Store mode in React context or state management
- Pass mode to order submission API calls
- Backend already supports `execution_mode: "shadow" | "live"` in DecisionInput

---

## Tech Stack Reference

```
testudo-web/
├── apps/web/src/
│   ├── components/     # UI components
│   ├── contexts/       # React contexts
│   ├── pages/          # Page components
│   └── services/       # API calls
├── package.json        # Uses React, Vite, TailwindCSS
```

**Commands:**
```bash
cd testudo-web
bun install
bun run dev      # Start dev server
bun run build    # Build
bun run lint     # Lint
```

---

## Suggested Component Structure

```tsx
// ModeToggle.tsx
interface ModeToggleProps {
  mode: 'shadow' | 'live';
  onModeChange: (mode: 'shadow' | 'live') => void;
}

// ModeIndicator.tsx - Badge showing current mode

// LiveModeConfirmDialog.tsx - Confirmation modal

// TradingModeContext.tsx - Context provider
```

---

## Success Criteria

- [ ] Toggle switch renders with Shadow/Live options
- [ ] Switching to Live shows confirmation dialog
- [ ] User must explicitly confirm to enable Live mode
- [ ] Visual badge shows current mode (green=shadow, red=live)
- [ ] Badge visible on Trade page
- [ ] Mode persists during session
- [ ] Order API calls include correct execution_mode
- [ ] All existing tests still pass

---

## Testing

```bash
cd testudo-web
bun run lint     # No errors
bun run build    # Builds successfully
```

Manual testing:
1. Load app - should default to Shadow mode (green badge)
2. Click toggle to Live - confirmation dialog appears
3. Cancel - stays in Shadow mode
4. Confirm - switches to Live (red badge)
5. Toggle back to Shadow - no confirmation needed
6. Place test order - verify execution_mode in network request

---

## Notes

- This is a **frontend-only** task - backend already supports execution modes
- The DecisionLoop in `router/src/decision_loop.rs` accepts `ExecutionMode::Shadow` or `ExecutionMode::Live`
- Focus on clear UX - traders must KNOW when they're in live mode
