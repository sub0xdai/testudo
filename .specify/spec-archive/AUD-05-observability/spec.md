# Specification: Observability Stack

**Spec ID:** AUD-05-observability
**Date:** 2026-03-07
**Status:** Complete
**Class:** Audit
**Phase:** 2 (Reliability)
**Audit Refs:** Structured logging, Prometheus metrics, Grafana dashboards

---

## Overview

Add production observability to the Testudo backend. Currently there are no structured logs, no metrics exporters, no correlation IDs, and no dashboards. Production failures cannot be traced across the distributed service boundary (extension → router → CCXT sidecar → exchange).

**Current state:**
- Logging uses `log::info/warn/error` with unstructured text — not machine-parseable
- No request correlation IDs — cannot trace a single trade through router → sidecar → exchange
- No Prometheus metrics exported — zero visibility into order latency, error rates, active positions
- No Grafana dashboards — no alerting on degradation

**Target state:**
- All logs emitted as structured JSON with timestamp, level, correlation_id, user_id, and context fields
- Every HTTP request gets a unique `X-Request-Id` propagated to CCXT sidecar calls
- Prometheus metrics endpoint at `/metrics` exports order, latency, error, and connection metrics
- 3 Grafana dashboards: Orders, Errors, Infrastructure

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | Replace `env_logger` with `tracing` + `tracing-subscriber` for structured JSON logging | High | Router |
| FR-2 | Add `tracing-actix-web` middleware for automatic request/response span creation | High | Router |
| FR-3 | Generate `X-Request-Id` (UUID) per request, propagate to CCXT sidecar HTTP calls | High | Router / CCXT Client |
| FR-4 | Add `actix-web-prom` or manual Prometheus endpoint at `/metrics` | High | Router |
| FR-5 | Export metrics: `testudo_orders_total` (counter, labels: side, status), `testudo_order_latency_seconds` (histogram), `testudo_active_positions` (gauge), `testudo_ws_connections` (gauge), `testudo_errors_total` (counter, labels: endpoint, status_code) | High | Router / Services |
| FR-6 | Export sidecar metrics: `testudo_sidecar_requests_total`, `testudo_sidecar_latency_seconds`, `testudo_sidecar_pool_size` | Medium | CCXT Sidecar |
| FR-7 | Create Grafana dashboard JSON: Orders (rate, latency p50/p99, by exchange), Errors (rate by endpoint, 5xx ratio), Infrastructure (connections, pool size, memory) | Medium | Ops |
| FR-8 | Add structured fields to critical log paths: fill detection, order cancellation, rehydration, trade creation | High | Router / Services |

---

## Technical Implementation

### 1) Structured Logging (FR-1, FR-2)

```toml
# Cargo.toml additions
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-actix-web = "0.7"
```

```rust
// main.rs initialization
tracing_subscriber::fmt()
    .json()
    .with_env_filter(EnvFilter::from_default_env())
    .with_target(true)
    .with_thread_ids(true)
    .init();

// Actix middleware
App::new()
    .wrap(tracing_actix_web::TracingLogger::default())
```

### 2) Request Correlation (FR-3)

```rust
// Middleware or extractor that generates/extracts X-Request-Id
pub struct RequestId(pub String);

impl FromRequest for RequestId {
    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        let id = req.headers()
            .get("X-Request-Id")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        ok(RequestId(id))
    }
}

// Propagate to CCXT sidecar calls
client.post(url)
    .header("X-Request-Id", &request_id)
    .send().await
```

### 3) Prometheus Metrics (FR-4, FR-5)

```rust
use prometheus::{IntCounterVec, HistogramVec, IntGauge, Registry};

lazy_static! {
    pub static ref ORDERS_TOTAL: IntCounterVec = IntCounterVec::new(
        Opts::new("testudo_orders_total", "Total orders placed"),
        &["side", "status"]
    ).unwrap();

    pub static ref ORDER_LATENCY: HistogramVec = HistogramVec::new(
        HistogramOpts::new("testudo_order_latency_seconds", "Order placement latency")
            .buckets(vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
        &["exchange"]
    ).unwrap();

    pub static ref ACTIVE_POSITIONS: IntGauge = IntGauge::new(
        "testudo_active_positions", "Current active positions"
    ).unwrap();
}

// /metrics endpoint
async fn metrics() -> HttpResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(buffer)
}
```

### 4) CCXT Sidecar Metrics (FR-6)

Add `prom-client` to the Node.js sidecar:

```javascript
const { collectDefaultMetrics, Counter, Histogram, register } = require('prom-client');
collectDefaultMetrics();

const sidecarRequests = new Counter({
    name: 'testudo_sidecar_requests_total',
    help: 'Total sidecar requests',
    labelNames: ['endpoint', 'status']
});

app.get('/metrics', async (req, res) => {
    res.set('Content-Type', register.contentType);
    res.end(await register.metrics());
});
```

---

## Verification

```bash
cd testudo-exchange && cargo clippy --all-targets && cargo test
# Verify metrics endpoint
curl http://localhost:8080/metrics | grep testudo_
```

- [ ] All logs output as JSON with timestamp, level, target
- [ ] Requests have X-Request-Id in logs and response headers
- [ ] /metrics endpoint returns Prometheus format
- [ ] Order creation increments testudo_orders_total
- [ ] Order latency recorded in histogram
- [ ] CCXT sidecar exposes /metrics
- [ ] All existing tests still pass
