export interface Settings {
  backendUrl: string;
  wsUrl: string;
  executionMode: "paper" | "live";
}

export interface AuthTokens {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export interface LoginResponse {
  user: { id: string; email: string };
  tokens: AuthTokens;
}

export interface TradePayload {
  symbol: string;
  side: "LONG" | "SHORT";
  entry: number;
  stop: number;
  target: number;
  timeframe: string;
}

export interface BackendResponse {
  success: boolean;
  data?: unknown;
  error?: string | null;
}

export type WsState = "disconnected" | "connecting" | "connected";
