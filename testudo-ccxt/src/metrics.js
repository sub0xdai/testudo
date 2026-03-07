'use strict';

// AUD-05 FR-6: Prometheus metrics for the CCXT sidecar.

const { Counter, Histogram, Gauge, Registry, collectDefaultMetrics } = require('prom-client');

const register = new Registry();

// Collect Node.js default metrics (GC, event loop lag, memory)
collectDefaultMetrics({ register });

/** Total sidecar HTTP requests (labels: endpoint, status) */
const sidecarRequestsTotal = new Counter({
  name: 'testudo_sidecar_requests_total',
  help: 'Total sidecar HTTP requests',
  labelNames: ['endpoint', 'status'],
  registers: [register],
});

/** Sidecar request latency in seconds (labels: endpoint) */
const sidecarLatencySeconds = new Histogram({
  name: 'testudo_sidecar_latency_seconds',
  help: 'Sidecar request latency in seconds',
  labelNames: ['endpoint'],
  buckets: [0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
  registers: [register],
});

/** Current exchange client pool size */
const sidecarPoolSize = new Gauge({
  name: 'testudo_sidecar_pool_size',
  help: 'Current CCXT exchange client pool size',
  registers: [register],
});

module.exports = {
  register,
  sidecarRequestsTotal,
  sidecarLatencySeconds,
  sidecarPoolSize,
};
