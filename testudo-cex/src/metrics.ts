/**
 * Prometheus metrics for testudo-cex sidecar.
 */

gN { Registry, Counter, Histogram } from "prom-client";

export const registry = new Registry();

export const httpRequestsTotal = new Counter({
  name: "testudo_cex_http_requests_total",
  help: "Total HTTP requests",
  labelNames: ["method", "route", "status"],
  registers: [registry],
});

export const httpRequestDuration = new Histogram({
  name: "testudo_cex_http_request_duration_seconds",
  help: "HTTP request duration in seconds",
  labelNames: ["method", "route"],
  registers: [registry],
});
