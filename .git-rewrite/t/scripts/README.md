# Testudo Exchange Management Scripts

This directory contains professional-grade scripts for managing the Testudo Exchange services.

## Scripts Overview

### 🚀 `start-exchange.sh`
Gracefully starts all exchange services in the correct order.

```bash
# Basic usage
./start-exchange.sh

# Advanced options
./start-exchange.sh --background --skip-build
./start-exchange.sh --reset-db --verbose
```

**Options:**
- `--background, -b`: Run services in background
- `--skip-build, -s`: Skip compilation step
- `--reset-db, -r`: Reset database (drop and recreate)
- `--verbose, -v`: Verbose output
- `--help, -h`: Show help

### 🛑 `stop-exchange.sh`
Gracefully stops all exchange services.

```bash
# Basic usage
./stop-exchange.sh

# Advanced options
./stop-exchange.sh --keep-infra --force
```

**Options:**
- `--keep-infra, -k`: Keep infrastructure (PostgreSQL) running
- `--force, -f`: Force shutdown (SIGKILL) if graceful fails
- `--quiet, -q`: Suppress output messages
- `--help, -h`: Show help

### 📊 `status-exchange.sh`
Shows the status of all exchange services.

```bash
# Basic usage
./status-exchange.sh

# Advanced options
./status-exchange.sh --watch --compact
```

**Options:**
- `--watch, -w`: Continuously watch status (refresh every 2s)
- `--compact, -c`: Compact output format
- `--help, -h`: Show help

### 📋 `logs-exchange.sh`
View and manage logs from all exchange services.

```bash
# Basic usage
./logs-exchange.sh

# View specific service
./logs-exchange.sh router
./logs-exchange.sh frontend

# Follow logs in real-time
./logs-exchange.sh --follow
./logs-exchange.sh --follow engine

# Advanced options
./logs-exchange.sh --lines 100 --since '1h ago'
```

**Services:** `router`, `engine`, `ws-stream`, `db-processor`, `frontend`, `docker`, `all`

**Options:**
- `--follow, -f`: Follow logs in real-time
- `--lines, -n LINES`: Number of lines to show (default: 50)
- `--since, -s TIME`: Show logs since timestamp
- `--all, -a`: Show all available logs summary
- `--help, -h`: Show help

## Quick Start

1. **Start the exchange:**
   ```bash
   ./scripts/start-exchange.sh --background
   ```

2. **Check status:**
   ```bash
   ./scripts/status-exchange.sh
   ```

3. **View logs:**
   ```bash
   ./scripts/logs-exchange.sh --follow
   ```

4. **Stop the exchange:**
   ```bash
   ./scripts/stop-exchange.sh
   ```

## Service Architecture

The scripts manage these services:

### Infrastructure
- **PostgreSQL** (port 5000): Database for trade history

### Backend Services
- **Router** (port 8080): API Gateway for REST endpoints
- **Engine**: Order matching engine (in-memory order books)
- **WebSocket Stream** (port 4000): Real-time market data
- **Database Processor**: Async database operations

### Frontend
- **Development Server** (port 5173): React/TypeScript trading interface

## Configuration

### Environment Variables
Copy `.env.example` to `.env` and configure:

```bash
cp .env.example .env
```

Key variables:
- `DATABASE_URL`: PostgreSQL connection string
- `SERVER_ADDR`: API gateway address
- `WS_STREAM_URL`: WebSocket server address

### Process Management
- PIDs stored in `scripts/.pids/`
- Logs stored in `scripts/.pids/*.log`
- Automatic cleanup of old logs (7+ days)

## Features

### Safety & Reliability
- ✅ Dependency checking (Docker, Rust, Node.js, etc.)
- ✅ Port conflict detection
- ✅ Graceful shutdown with SIGTERM → SIGKILL fallback
- ✅ Health checks and service readiness waiting
- ✅ Process tracking with PID files
- ✅ Automatic cleanup and error handling

### Monitoring & Debugging
- ✅ Real-time status monitoring
- ✅ Colored log output with service prefixes
- ✅ Service uptime tracking
- ✅ Port status monitoring
- ✅ Docker container status

### Development Workflow
- ✅ Background/foreground execution modes
- ✅ Skip build for faster iteration
- ✅ Database reset capabilities
- ✅ Verbose logging options
- ✅ Infrastructure preservation options

## Troubleshooting

### Common Issues

**Port conflicts:**
```bash
# Check what's using the port
lsof -i :8080

# Force stop everything
./scripts/stop-exchange.sh --force
```

**Services won't start:**
```bash
# Check prerequisites
./scripts/start-exchange.sh --verbose

# Check detailed status
./scripts/status-exchange.sh

# View logs for errors
./scripts/logs-exchange.sh --all
```

**Database issues:**
```bash
# Reset database
./scripts/start-exchange.sh --reset-db
```

### Manual Cleanup
If scripts fail to manage services:

```bash
# Kill all exchange processes
pkill -f "cargo run --bin"
pkill -f "bun"

# Stop Docker containers
cd testudo-exchange/docker && docker compose down

# Clean PID files
rm -f scripts/.pids/*.pid
```

## Development

To modify the scripts:

1. All scripts follow the same structure:
   - Argument parsing with help
   - Colored logging functions
   - Main execution function
   - Signal handling for cleanup

2. PID management:
   - Store PIDs in `scripts/.pids/`
   - Track process state
   - Clean up on exit

3. Testing:
   - Test with different flag combinations
   - Verify graceful shutdown
   - Check error handling