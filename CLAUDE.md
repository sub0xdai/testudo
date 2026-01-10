# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Structure

This is a multi-component cryptocurrency exchange project with three main components:

- **testudo-exchange/**: Rust backend - High-performance matching engine and API server
- **testudo-web/**: TypeScript/React frontend - Trading interface using Turbo monorepo
- **testudo-ops/**: Kubernetes infrastructure - Production deployment configurations

## Development Commands

### Web Frontend (testudo-web/)
```bash
cd testudo-web
bun install            # Install dependencies
bun run dev            # Start development server
bun run build          # Build for production
bun run lint           # Run ESLint
bun run format         # Format code with Prettier
```

### Rust Backend (testudo-exchange/)
```bash
cd testudo-exchange
cargo build             # Build the project
cargo run              # Run the exchange
cargo test             # Run tests
cargo fmt              # Format code
cargo clippy           # Run linter
```

### Individual web app development:
```bash
cd testudo-web/apps/web
bun run dev            # Start Vite dev server
bun run build          # Build with TypeScript check
bun run lint           # Run ESLint
bun run preview        # Preview production build
```

## Architecture Overview

### Exchange Backend (Rust)
- **In-memory order matching engine** with price-time priority
- **WebSocket streams** for real-time market data
- **Redis pub/sub** for message queuing and caching
- **PostgreSQL** for persistent storage
- **Modular crate structure**:
  - `engine/`: Core matching engine and order book
  - `ws-stream/`: WebSocket handling
  - `db-processor/`: Database operations
  - `redis/`: Redis client utilities
  - `router/`: HTTP API routing
  - `common_utils/`: Shared utilities

### Web Frontend (TypeScript/React)
- **Turbo monorepo** with workspaces
- **Vite** for fast development and building
- **React Router** for navigation
- **Lightweight Charts** for trading interface
- **Axios** for API communication
- **Tailwind CSS** for styling

### Infrastructure (Kubernetes)
- **GKE deployment** with ArgoCD GitOps
- **NGINX Ingress** with TLS certificates
- **Prometheus + Grafana** monitoring
- **Sealed Secrets** for secure credential management

## Key Files to Understand

- `testudo-exchange/crates/engine/src/engine/orderbook.rs`: Core order matching logic
- `testudo-web/apps/web/src/pages/Trade.tsx`: Main trading interface
- `testudo-web/apps/web/src/components/depth/OrderBook.tsx`: Order book display
- `testudo-ops/postgres-db/`: Database configurations
- `.synapse.yml`: Project metadata for Synapse System integration

## Testing

The Rust backend uses standard `cargo test`. The web frontend uses the testing framework configured in each package. Always run tests before committing changes to ensure the matching engine and trading interface work correctly.

## Environment Setup

The exchange requires:
- **Redis** for caching and pub/sub
- **PostgreSQL** for persistent storage
- **Bun** for web development (faster alternative to Node.js)
- **Rust** with recent stable toolchain

See individual README files in each component directory for detailed setup instructions.