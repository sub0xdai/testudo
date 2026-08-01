# Unified Terminal Architecture: Findings & Strategy

## 1. Architectural Findings
The current `testudo-extension` architecture is built on a "Web App" mental model (Auth View ↔ Main View). This creates artificial boundaries between the user and the trading environment.

### Data & State Fragmentation
- **Auth Separation:** `App.tsx` uses a binary `Switch`. This unmounts the dashboard entirely when unauthenticated, preventing any "Terminal" persistence or background data preparation.
- **Account Isolation:** The `Account` tab in `MainView` isolates critical telemetry (wallet address, exposure, detailed balance) into a separate view. Traders lose situational awareness when checking account health.
- **Pairing UX:** The 6-digit OTP is treated as a full-page "login" barrier rather than a secure "PIN pad" that unlocks the desk.

## 2. The Unified Desk Strategy
Shift the architecture to a **Persistent Terminal** model. The UI shell is always present; only the *capabilities* and *data visibility* are gated by the pairing state.

### High-Level Architectural Shifts
- **Single Root View:** `App.tsx` always mounts the `MainView`. The dashboard is the foundation, not a destination.
- **Gateway Pattern:** The 6-digit Pairing flow moves from a standalone view to a component (`PairingGateway`) that overlays the Desk's workspace.
- **Auth as a Toggle:** The "Connect" button in the header/desk becomes the terminal's main power switch—triggering the pairing gateway locally instead of redirecting externally.

### Layout Consolidation
- **Telemetry Integration:** Wallet address and exposure metrics move to the `HeaderBar` and `StatusBar` for permanent visibility.
- **Tab Elimination:** Remove the `Account` tab. Detailed balance stats are merged into the top-level `BalancePanel`.
- **Workspace Focus:** The `TabBar` is reduced to `Trade` and `Positions`, maximizing screen real estate for active execution.

## 3. Implementation Roadmap
1. **Unify Root:** Eliminate `App.tsx` routing. Make `MainView` the sole orchestrator.
2. **Inject Gateway:** Refactor `PairView` logic into a `PairingGateway` component that masks the `Trade`/`Positions` panels.
3. **Migrate Telemetry:** Extract account data from the `Account` tab and re-distribute it to the Header/Balance Panel.
4. **Direct Connect:** Wire the "Connect" button to activate the `PairingGateway` state directly.



