import type { Request, Response, NextFunction } from "express";

const SIDECAR_PSK = process.env.SIDECAR_PSK;

export function pskGuard(req: Request, res: Response, next: NextFunction) {
  // Always allow health checks (K8s liveness probes)
  if (req.path === "/health") return next();

  // SEC-03: Fail closed — reject all requests when PSK is not configured
  if (!SIDECAR_PSK) {
    return res.status(503).json({ error: "PSK not configured" });
  }

  if (req.headers["x-internal-secret"] !== SIDECAR_PSK) {
    return res.status(401).json({ error: "unauthorized" });
  }
  next();
}
