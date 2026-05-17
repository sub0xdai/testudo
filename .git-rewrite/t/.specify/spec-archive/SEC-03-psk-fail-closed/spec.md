# Specification: Make CCXT Sidecar PSK Guard Fail Closed

**Spec ID:** SEC-03-psk-fail-closed
**Date:** 2026-03-30
**Status:** Complete
**Class:** Infrastructure / Security
**Priority:** P1 — Missing env var silently disables sidecar authentication
**Depends on:** None
**Series:** SEC-01 through SEC-04 (Security review remediation)

---

## Problem Statement

The PSK (Pre-Shared Key) middleware in `testudo-cex/src/middleware/psk.ts` (line 6) implements fail-open behavior:

```typescript
if (!SIDECAR_PSK) return next();
```

When `SIDECAR_PSK` is not set, every request passes through without authentication. This is the only network-level authentication for the sidecar — if it's silently disabled by a missing environment variable, any service with network access to port 3100 can call all sidecar endpoints.

While the sidecar requires caller-supplied exchange credentials per-request (mitigating direct exploitation), the PSK is defense-in-depth. In a lateral movement scenario where credentials are obtained from another breach vector, the PSK is the last barrier. A fail-open PSK converts a "need PSK + credentials" attack into a "need credentials only" attack.

The fix is to fail closed: reject all requests (except `/health`) when PSK is not configured, or refuse to start the server entirely.

---

## User Stories

- **As a platform operator**, I want the sidecar to refuse requests when PSK is misconfigured, so that a missing env var doesn't silently disable authentication.
- **As a security engineer**, I want defense-in-depth controls to fail closed, so that misconfigurations are loud failures rather than silent bypasses.

---

## Functional Requirements

| ID | Requirement | Priority | Subsystem |
|----|-------------|----------|-----------|
| FR-1 | PSK middleware must reject all non-health requests when `SIDECAR_PSK` is unset | High | CEX Sidecar |
| FR-2 | Server should log a clear warning at startup when `SIDECAR_PSK` is not configured | Medium | CEX Sidecar |
| FR-3 | `/health` endpoint must remain accessible without PSK for liveness probes | Medium | CEX Sidecar |

---

## Technical Implementation

### Vertical Checkpoints

| Checkpoint | Scope | Validates |
|------------|-------|-----------|
| CP-1 | Change PSK middleware to fail closed + startup warning | Requests without PSK configured return 503 |
| CP-2 | Exempt `/health` from PSK check | K8s liveness probes still work without PSK |

### Changes to `testudo-cex/src/middleware/psk.ts`

```typescript
import { Request, Response, NextFunction } from "express";

const SIDECAR_PSK = process.env.SIDECAR_PSK;

export function pskGuard(req: Request, res: Response, next: NextFunction) {
  // Allow health checks without PSK
  if (req.path === "/health") return next();

  // Fail closed: reject all requests when PSK is not configured
  if (!SIDECAR_PSK) {
    return res.status(503).json({ error: "PSK not configured" });
  }

  const token = req.headers["x-psk"] as string;
  if (token !== SIDECAR_PSK) {
    return res.status(401).json({ error: "Invalid PSK" });
  }

  next();
}
```

### Startup Warning

In the sidecar's main entry point, add a startup check:

```typescript
if (!process.env.SIDECAR_PSK) {
  console.warn("WARNING: SIDECAR_PSK not set — all non-health requests will be rejected");
}
```

### Paved Roads

- The existing `pskGuard` middleware already handles the PSK check correctly when the env var IS set — only the missing-env-var path needs to change.
- `/health` is already a separate route in the sidecar.

### Files

- `testudo-cex/src/middleware/psk.ts` — Change fail-open to fail-closed, exempt `/health`
- `testudo-cex/src/index.ts` (or main entry) — Add startup warning log

### Dependencies Added

None.

---

## Acceptance Criteria

- [ ] Requests to any endpoint (except `/health`) return 503 when `SIDECAR_PSK` is unset
- [ ] `/health` returns 200 regardless of PSK configuration
- [ ] Requests with valid PSK header continue to work normally
- [ ] Requests with invalid PSK header return 401
- [ ] Startup logs include a warning when `SIDECAR_PSK` is not set
- [ ] `bun run build` passes (or equivalent sidecar build command)

---

## Risks

1. **Existing deployments without PSK** — Any deployment currently running without `SIDECAR_PSK` will start rejecting requests. Mitigation: This is the desired behavior; ensure all deployment configs (docker-compose, K8s manifests) set `SIDECAR_PSK`.
2. **Router-sidecar connection breaks** — If the router doesn't send PSK, it will be rejected. Mitigation: Verify `testudo-exchange` CCXT client sends the PSK header (check `cex_client.rs`).

---

## Completion Signal

This spec is complete when:
1. PSK middleware fails closed when `SIDECAR_PSK` is unset
2. `/health` remains accessible without PSK
3. Startup warning is logged
4. All acceptance criteria met
5. Code committed to master
