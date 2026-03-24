import type { Request, Response, NextFunction } from "express";

const SIDECAR_PSK = process.env.SIDECAR_PSK;

export function pskGuard(req: Request, res: Response, next: NextFunction) {
  if (!SIDECAR_PSK) return next();
  if (req.path === "/health") return next();
  if (req.headers["x-internal-secret"] !== SIDECAR_PSK) {
    return res.status(401).json({ error: "unauthorized" });
  }
  next();
}
