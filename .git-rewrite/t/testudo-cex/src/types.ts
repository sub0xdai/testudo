/**
 * Shared types and request/response shapes.
 */

export interface HealthResponse {
  ok: boolean;
}

export interface ErrorResponse {
  error: string;
}

export interface OrderRequest {
  exchange: string;
  symbol: string;
  side: "buy" | "sell";
  type: "limit" | "market";
  amount: string;
  price?: string;
  params?: Record<string, unknown>;
}

export interface OrderResponse {
  id: string;
  clientOrderId?: string;
  status: string;
  symbol: string;
  side: string;
  type: string;
  amount: string;
  price: string;
  filled: string;
  remaining: string;
}
