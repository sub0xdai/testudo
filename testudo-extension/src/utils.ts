import type { Settings } from "./types";

// --- Constants ---

export const DEFAULT_SETTINGS: Settings = {
  backendUrl: "http://localhost:8080",
  wsUrl: "ws://localhost:4000",
  executionMode: "paper",
};

export const PAPER_USER_ID = "00000000-0000-0000-0000-000000000001";

export const WEB_APP_URL = "http://localhost:3001";

export const WS_MAX_RECONNECT_DELAY = 30000;
export const WS_BASE_RECONNECT_DELAY = 1000;

// --- Symbol Normalization (EXT-04 FR-3) ---

export const QUOTE_CURRENCIES = [
  "USDT", "USDC", "BUSD", "TUSD", "FDUSD",
  "BTC", "ETH", "BNB", "DAI",
  "EUR", "GBP", "USD",
];

export function normalizeSymbol(tvSymbol: string): string {
  const upper = tvSymbol.toUpperCase();
  for (const quote of QUOTE_CURRENCIES) {
    if (upper.endsWith(quote) && upper.length > quote.length) {
      const base = upper.slice(0, -quote.length);
      return `${base}_${quote}`;
    }
  }
  return upper;
}

// --- Position Sizing ---

export function calculateQuantity(
  entry: number,
  stop: number,
  riskAmount: number = 100,
): number {
  const stopDistance = Math.abs(entry - stop);
  const quantity = stopDistance > 0 ? riskAmount / stopDistance : 0.001;
  return Math.round(quantity * 1e8) / 1e8;
}

// --- Side Mapping ---

export function mapSide(side: "LONG" | "SHORT"): "buy" | "sell" {
  return side === "LONG" ? "buy" : "sell";
}

// --- Token Refresh Delay ---

export function calculateRefreshDelay(expiresIn: number): number {
  return Math.max(10, expiresIn - 60) * 1000;
}

// --- WS Reconnect Delay ---

export function nextReconnectDelay(current: number): number {
  return Math.min(current * 2, WS_MAX_RECONNECT_DELAY);
}
