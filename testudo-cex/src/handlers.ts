/**
 * HTTP route handlers — same endpoints as testudo-ccxt.
 * CEX-04 will implement full endpoint compatibility.
 */

import type { Request, Response } from "express";

export function handleBalance(_req: Request, res: Response) {
  res.status(501).json({ error: "not implemented" });
}

export function handleOrder(_req: Request, res: Response) {
  res.status(501).json({ error: "not implemented" });
}

export function handleCancelOrder(_req: Request, res: Response) {
  res.status(501).json({ error: "not implemented" });
}

export function handleEditOrder(_req: Request, res: Response) {
  res.status(501).json({ error: "not implemented" });
}

export function handleOpenOrders(_req: Request, res: Response) {
  res.status(501).json({ error: "not implemented" });
}

export function handlePosition(_req: Request, res: Response) {
  res.status(501).json({ error: "not implemented" });
}

export function handleLeverage(_req: Request, res: Response) {
  res.status(501).json({ error: "not implemented" });
}
