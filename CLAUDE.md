# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Structure

This is a multi-component cryptocurrency exchange project with three main components:

- **testudo-exchange/**: Rust backend - High-performance matching engine and API server
- **testudo-web/**: TypeScript/React frontend - Landing site and account management (Vite + Tailwind)
- **testudo-ops/**: Kubernetes infrastructure - Production deployment configurations

## Development Commands

### Web Frontend (testudo-web/)
```bash
cd testudo-web
bun install            # Install dependencies
bun run dev            # Start Vite dev server
bun run build          # Build with TypeScript check
bun run preview        # Preview production build
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
- **Vite** for fast development and building
- **React 18** with TypeScript
- **Tailwind CSS** for styling
- Landing site with brutalist dark theme (Unbounded + Space Mono fonts)

### Infrastructure (Kubernetes)
- **GKE deployment** with ArgoCD GitOps
- **NGINX Ingress** with TLS certificates
- **Prometheus + Grafana** monitoring
- **Sealed Secrets** for secure credential management

## Key Files to Understand

- `testudo-exchange/crates/engine/src/engine/orderbook.rs`: Core order matching logic
- `testudo-web/src/App.tsx`: Landing site entry point
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