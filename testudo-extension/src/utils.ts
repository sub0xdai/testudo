import type { Settings } from "./types";

// --- Exchange Type Derivation ---

const DEX_EXCHANGES = new Set(["hyperliquid"]);

export type ExchangeMode = "cex" | "dex";

export function getExchangeType(exchangeName: string): ExchangeMode {
  return DEX_EXCHANGES.has(exchangeName.toLowerCase()) ? "dex" : "cex";
}

// --- Constants ---

export const DEFAULT_SETTINGS: Settings = {
  backendUrl: "http://localhost:8080",
  wsUrl: "ws://localhost:4000",
};

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
      // No crypto exchange trades raw USD perps — upgrade to USDT
      const normalizedQuote = quote === "USD" ? "USDT" : quote;
      return `${base}_${normalizedQuote}`;
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
