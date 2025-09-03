# Imperium: API Server & Command Interface

This crate is the backend API server for the Testudo platform. It provides the core command and control interface, serving data to the frontend and executing trading operations. It is designed for high performance, security, and reliability.

## Core Responsibilities
- **API Endpoints**: Exposes a secure RESTful API for all trading, account, and risk management operations.
- **Authentication**: Implements OIDC-based authentication to secure the platform.
- **Real-Time Communication**: Provides a WebSocket interface for streaming live market data and account updates.
- **Orchestration**: Connects the other core crates (`disciplina`, `formatio`, `prudentia`) to fulfill user requests.

---

## Frontend Architecture: Leptos (Rust/WASM)
The `imperium` backend serves a high-performance frontend built with the **Leptos** framework, compiled to **WebAssembly (WASM)**. This provides end-to-end type safety and maximizes performance.

- **Framework**: Leptos with Client-Side Rendering (CSR).
- **Build Tool**: Trunk, optimized for WASM Single-Page Applications (SPAs).
- **UI Philosophy**: A "Terminal-First" design inspired by Bloomberg Terminals, prioritizing information density and performance.
- **Styling**: Tailwind CSS, configured with a monochromatic theme with subtle neon accents.
- **State Management**: Leptos's native, fine-grained reactive system (signals and contexts).
- **Authentication**: OIDC flow managed by the `leptos_oidc` crate, with JWTs stored securely in memory.

### Key User Flows
1.  **Authentication**: Secure login via a self-hosted OIDC provider (e.g., Keycloak).
2.  **Onboarding**: A mandatory, one-time wizard to configure CEX API keys and a user risk profile.
3.  **Trade Setup**: A drag-and-drop tool on the TradingView chart for setting entry, stop, and target levels.
4.  **Sizing & Execution**: The UI displays real-time position sizing from the `disciplina` engine and communicates risk assessments from `prudentia` before execution.
5.  **Monitoring**: Real-time P&L and system status updates via WebSockets.

---

## Non-Negotiable Requirements 📜

### Performance Budgets
- **API Response Time**: <50ms for most endpoints.
- **WebSocket Latency**: <100ms for message delivery.
- **Trade Execution Loop**: <200ms from API call to exchange confirmation.

### Security Mandates
- **Authentication**: All sensitive API endpoints must be protected by a valid OIDC-issued JWT.
- **API Keys**: User CEX keys must be encrypted at rest.
- **Data Storage**: No sensitive data is ever stored in `localStorage` or `sessionStorage` on the frontend.

---

## Testing Strategy
- **Unit & Integration Tests**: Backend endpoints and logic are tested with `tokio::test`.
- **Frontend Testing**: The Leptos frontend is tested using its native testing utilities, focusing on component behavior and reactivity.

---

## Key Commands

### Primary Test Command (TDD Guard Enabled)
Use this command for all development. It enforces the Red-Green-Refactor cycle.
```
cargo nextest run | tdd-guard-rust --passthrough
```

### Additional Commands
- **Run backend tests**: `cargo test --package imperium`
- **Start backend server**: `cargo run --bin testudo`
- **Build frontend**: `cd frontend && trunk build --release`
- **Serve frontend (dev)**: `cd frontend && trunk serve --open`
