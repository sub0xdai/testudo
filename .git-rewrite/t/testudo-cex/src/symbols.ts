/**
 * Symbol normalization between Rust backend format and safe-cex exchange format.
 *
 * Backend uses `BTC_USDT` (underscore-separated base_quote).
 * safe-cex uses `BTCUSDT` (concatenated, exchange-native format).
 */

import type { Market } from "safe-cex/dist/types";

/** Known quote currencies, ordered longest-first to avoid greedy prefix matching. */
const KNOWN_QUOTES = ["USDT", "USDC", "BUSD"] as const;

/**
 * Convert backend symbol to exchange format.
 * "BTC_USDT" → "BTCUSDT"
 */
export function toExchangeSymbol(backendSymbol: string): string {
  return backendSymbol.replace("_", "");
}

/**
 * Convert exchange symbol to backend format using market data for correct split.
 * "BTCUSDT" → "BTC_USDT"
 *
 * Uses market.base and market.quote for reliable splitting (handles edge cases
 * like 1000PEPEUSDT where greedy suffix stripping would fail).
 */
export function toBackendSymbol(
  exchangeSymbol: string,
  markets?: Market[]
): string {
  // Strategy 1: Use market data for precise split
  if (markets) {
    const market = markets.find((m) => m.symbol === exchangeSymbol);
    if (market) {
      return `${market.base}_${market.quote}`;
    }
  }

  // Strategy 2: Fallback to known quote suffix stripping
  for (const quote of KNOWN_QUOTES) {
    if (exchangeSymbol.endsWith(quote)) {
      const base = exchangeSymbol.slice(0, -quote.length);
      if (base.length > 0) {
        return `${base}_${quote}`;
      }
    }
  }

  // Strategy 3: Passthrough if no match
  return exchangeSymbol;
}
