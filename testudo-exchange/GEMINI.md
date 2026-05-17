# Gemini Project Overview: testudo-exchange

This directory contains the core of the exchange, written in Rust for maximum performance and low-latency trading. It uses an in-memory order book, real-time WebSocket updates, and a backend powered purely by PostgreSQL (using SKIP LOCKED queues and LISTEN/NOTIFY for real-time operations).

## Key Components:

*   **Rust Backend**: Handles REST API and WebSocket requests.
*   **PostgreSQL**: Unified datastore handling persistent storage, message queues (pg_queue), and high-performance caching.
*   **WebSocket Layer**: Streams market data and order updates to clients via tokio-tungstenite.
*   **Matching Engine**: In-memory order book and trade execution with Shadow Engine (paper trading) support.

## Local Development:

To build and run the exchange locally:

```sh
cp .env.example .env
# Configure PostgreSQL credentials in .env
docker-compose up -d
cargo build
cargo run
```
