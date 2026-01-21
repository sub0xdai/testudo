# Project Overview

This repository contains a high-performance centralized cryptocurrency exchange, built with Rust, and deployed on a production-grade Kubernetes cluster on Google Cloud Platform (GCP). The project is divided into three main parts:

*   **`testudo-exchange`**: The core of the exchange, written in Rust for maximum performance and low-latency trading. It uses an in-memory order book, real-time WebSocket updates, and a backend powered by Redis and Postgres.
*   **`testudo-ops`**: Contains the infrastructure-as-code for deploying the exchange to a Kubernetes cluster. It uses GKE, ArgoCD for GitOps, NGINX Ingress, Sealed Secrets for secret management, and cert-manager for automated TLS certificates.
*   **`testudo-web`**: The web-based user interface for the exchange.

## Building and Running

### `testudo-exchange` (Rust Backend)

To build and run the exchange locally:

```sh
cd testudo-exchange
cp .env.example .env
# Configure Postgres and Redis credentials in .env
docker-compose up -d
cargo build
cargo run
```

### `testudo-ops` (Kubernetes)

The `testudo-ops` project uses ArgoCD for GitOps-based deployment to a GKE cluster. The main configuration files are located in the `argocd` directory. To deploy the exchange, you would typically apply these configurations to your ArgoCD instance.

### `testudo-web` (Web UI)

To run the web UI for local development:

```sh
cd testudo-web
bun install
bun run dev
```

## Development Conventions

*   **Backend (Rust)**: The backend is built with a focus on performance and memory safety, using asynchronous programming with Tokio and the Actix web framework.
*   **Infrastructure (Kubernetes)**: The infrastructure is managed declaratively using GitOps principles with ArgoCD. All Kubernetes configurations are stored in YAML files.
*   **Frontend (Web)**: The frontend is a modern web application, likely built with a popular JavaScript framework like React or Vue.js (based on the presence of `package.json`, `vite.config.ts`, etc.).
