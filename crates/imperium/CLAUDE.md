# Imperium: Command & Control Interface

This crate is the Progressive Web App interface for the Testudo platform. It provides a command center for traders to execute disciplined, systematic trades with a focus on performance, clarity, and security.

## Core Principles & UX
The interface is guided by a philosophy of disciplined minimalism.

- **Minimalism**: Remove cognitive load; focus on the critical decision.
- **Clarity**: Information hierarchy must match decision importance.
- **Speed**: All user interactions must feel instantaneous (<100ms response).
- **Discipline**: The interface must guide and enforce disciplined trading behavior.

### Key User Flows
1.  **Authentication**: Secure login via Clerk integration.
2.  **Trade Setup**: Drag-based entry/stop/target on TradingView charts.
3.  **Sizing**: Automatic, real-time display of Van Tharp position size.
4.  **Execution**: Visual risk confirmation before committing the trade.
5.  **Monitoring**: Real-time P&L analysis.

---

## Architecture & State Management
The application is built on a modern, performant stack.

### Component Architecture
- **Charting**: `TradingView Lightweight Charts` is the core of the trade setup interface.
- **PWA**: A Service Worker provides offline chart caching and push notifications for trade alerts.

### State Management
- **Server State**: `React Query` is used for all API data synchronization.
- **Client State**: `Zustand` is used for managing global UI state. The core store is defined by the `TradingStore` interface.
`
// Zustand store for global UI state
interface TradingStore {
  currentTradeSetup: TradeSetup | null;
  calculatedPositionSize: PositionSize | null;
  positions: Position[];
  accountEquity: Decimal;
  isTradeSetupActive: boolean;

  // Actions for state mutations
  setTradeSetup: (setup: TradeSetup) => void;
  executeTradeAction: (plan: ExecutionPlan) => Promise<void>;
}
`
---

## Non-Negotiable Requirements 📜

### Performance Budgets
- **First Contentful Paint**: < 3s
- **Chart Rendering**: < 16ms per frame (60fps)
- **Trade Execution UI**: < 200ms from user click to confirmation
- **Main Bundle Size**: < 500KB gzipped

### Security Mandates
- **API Keys**: Stored server-side only. **Never** expose in client-side code.
- **Authentication**: All sensitive actions must be protected by a valid Clerk session token.
- **Data Storage**: No sensitive data (e.g., full account equity, personal info) may be stored in `localStorage` or `sessionStorage`.
- **CSP**: A strict Content Security Policy must be in place to prevent XSS attacks.

### Accessibility Standards
- **Compliance**: Must meet WCAG 2.1 AA standards.
- **Keyboard Navigation**: All trading functions must be fully controllable via the keyboard.
- **Screen Readers**: All interactive elements and data displays must have proper ARIA labels.

---

## Testing Strategy
Testing must validate both individual components and complete user workflows.

### Component Testing (React Testing Library)
Components are tested from a user's perspective, focusing on interactions and rendered output.
`
test('calculates and displays position size on drag interaction', async () => {
  // 1. Render the TradingChart component with a mocked calculator.
  // 2. Simulate a drag event on the chart canvas.
  // 3. Assert that the position sizing function was called.
  // 4. Assert that the UI correctly displays the new position size and risk amount.
});
`
### End-to-End Testing 
test('completes a full trade execution workflow', async ({ page }) => {
  // 1. Login with a test account.
  // 2. Navigate to the trading interface.
  // 3. Simulate dragging entry/stop lines on the chart.
  // 4. Verify the position size UI updates correctly.
  // 5. Click the execute button.
  // 6. Assert that a trade confirmation notification appears.
});
```

---

## Key Commands

### Primary Test Command (TDD Guard Enabled)
Use this command for all development. It enforces the Red-Green-Refactor cycle.
```
cargo nextest run | tdd-guard-rust --passthrough
```

### Additional Commands
- **Run component tests**: `npm test`
- **Run E2E tests**: `npm run test:e2e`
- **Build for production**: `npm run build`
- **Start dev server**: `npm run dev`
`
