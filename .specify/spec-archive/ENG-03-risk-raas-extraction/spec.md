# Specification: Extract Risk Engine to Stateless RaaS Microservice

**Spec ID:** ENG-03-risk-raas-extraction
**Date:** 2026-04-24
**Status:** SUPERSEDED (2026-04-25) — retired in favour of ENG-04-async-order-pipeline
**Class:** Infrastructure / Microservice
**Priority:** P0 — Critical for decoupling the risk layer from the monolith to improve performance and security.
**Depends on:** None (first in series)
**Series:** ENG-03 through ENG-05 (Risk Layer Extraction)
**Superseded by:** ENG-04-async-order-pipeline

---

## Supersession Note (2026-04-25)

This spec was retired after investigation showed its core motivation was incorrect.

**Claim (problem statement):** *"the use of `Decimal` and `async` overhead in mathematical calculations adds a 'Risk Tax' to every trade"* — implying risk math is on the hot path and extraction would reduce user-visible latency.

**Reality observed in code:**
- `RiskService::new(config)` (`testudo-exchange/crates/common_utils/src/risk/service.rs:77`) holds only a `RiskConfig`. No sqlx, no reqwest, no async in `validate()`.
- Per-call cost is synchronous `rust_decimal` math, sub-microsecond.
- The actual hot-path tax is the **sidecar HTTP roundtrip** (5–20 ms happy path, multi-second on stalls), not risk math.

**Extraction would make things worse:**
- gRPC + TCP adds ~100 µs–1 ms on localhost, far more across zones.
- The reference implementation (`testudo-raas/src/risk.rs`) decodes f64 on the wire, runs `Decimal` math internally, re-encodes f64 — worst of both precision and performance regimes, and violates the project rule "never f64 for financial math".
- Adds a new network SPOF, new auth surface, new deploy target, without relieving any real bottleneck.

**The real hot-path fix is ENG-04.** Moving `DecisionLoop` into a queue worker (off the HTTP request path) removes whatever latency it has from user-visible response time without any extraction, and simultaneously fixes the actual bottleneck (sidecar-induced HTTP worker blocking).

If policy isolation, multi-tenancy, or edge deployment later become real requirements, a new spec should be written from that motivation — not this one, which conflates a non-bottleneck with a bottleneck.

**Artefacts retired with this spec:**
- `testudo-raas/` crate (scaffold only, no callers, ~220 LOC) — deleted.
- The 148-line reference risk engine in that crate was a partial reimplementation of `common_utils/risk/` (3112 LOC across 9 files) and did not include Kelly, the Conservative Wins MIN policy, or per-user config provisioning. It carried no value worth preserving.

---

## Problem Statement

The current risk management logic in `testudo-exchange` is a "Service" entangled within the monolith's architecture. It is coupled with database connections (`sqlx`), HTTP clients (`reqwest`), and complex domain types, which introduces unnecessary latency and security risks to the core decision loop.

This entanglement prevents the risk engine from being scaled independently or deployed at the edge. Furthermore, the use of `Decimal` and `async` overhead in mathematical calculations adds a "Risk Tax" to every trade. We need a high-performance, stateless, and logically isolated "math engine" that can evaluate risk in nanoseconds.

This spec establishes `testudo-raas` (Risk-as-a-Service) as a standalone gRPC microservice using Rust 2024, Tonic, and a pure functional core.

---

## User Stories

- **As a Systems Architect**, I want a stateless risk service isolated from the monolith, so that I can scale risk evaluation independently and reduce the blast radius of security vulnerabilities.
- **As a Quantitative Developer**, I want the risk engine to use pure mathematical functions with `f64` primitives, so that calculations are deterministic, high-performance, and audit-proof.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Establish gRPC interface with `ValidateOrder` RPC | High | Boundary |
| FR-2 | Implement pure mathematical functions for Kelly and Volatility Scaling | High | Core |
| FR-3 | Decouple internal mathematical domain from gRPC contract via `TryFrom` mapping | High | Boundary |
| FR-4 | Build optimized, distroless container for edge deployment | Medium | DevOps |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | gRPC server scaffold with `tonic` and `proto` | Network connectivity |
| CP-2 | Pure mathematical core extraction to `src/risk.rs` | Calculation accuracy |
| CP-3 | Airgap boundary implementation via `TryFrom` | Domain isolation |
| CP-4 | Containerization with multi-stage distroless build | Deployment readiness |

### Architecture & Isolation

The service is structured into three distinct layers:
1. **Boundary Layer**: Handles gRPC/HTTP2 communication and decodes payloads.
2. **Airgap Layer**: Uses `TryFrom` to map gRPC structs to internal mathematical types.
3. **Core Layer**: Pure, synchronous functions using `f64` for maximum performance.

```rust
// src/risk.rs - Pure mathematical core
pub struct InternalPortfolio {
    pub balance: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
}

pub fn calculate_final_size(
    portfolio: &InternalPortfolio,
    order: &InternalOrder,
    params: &RiskParams,
) -> f64 {
    // Math logic (Kelly, Volatility Adjusted, Fixed Fractional)
}
```

### Files

- `proto/risk_engine.proto` — gRPC contract definition
- `src/main.rs` — gRPC server and boundary mapping
- `src/risk.rs` — Pure functional risk engine
- `Containerfile` — Multi-stage distroless build
- `Cargo.toml` — Dependency management (Tonic, Prost, Tokio)

### Dependencies Added

- `tonic = "0.12"` — gRPC framework
- `prost = "0.13"` — Protocol Buffers implementation
- `tokio = { version = "1.0", features = ["full"] }` — Async runtime

---

## Acceptance Criteria

- [ ] gRPC server listens on `0.0.0.0:50051`.
- [ ] `ValidateOrder` returns correct `kelly_adjusted_size` based on pure math.
- [ ] Internal mathematical core has zero `async` or I/O dependencies.
- [ ] Verification command passes: `podman build -t testudo-raas:latest .`

---

## Risks

1. **Domain Leakage** — Protobuf types leaking into the math core. Mitigation: Use strict `TryFrom` mapping at the boundary.
2. **Precision Loss** — Moving from `Decimal` to `f64`. Mitigation: Verify that `f64` precision is sufficient for risk-sizing thresholds.

---

## Completion Signal

This spec is complete when:
1. gRPC service is functional and responding to requests.
2. Pure mathematical core is fully isolated in `src/risk.rs`.
3. Distroless container image is successfully built.
4. Code committed to master.
