/**
 * Symbol normalization between backend format and exchange format.
 * CEX-07 will implement full symbol mapping.
 */

/** Convert backend symbol (BTC_USDT) to exchange format (BTCUSDT) */
export function toExchangeSymbol(backendSymbol: string): string {
  return backendSymbol.replace("_", "");
}

/** Convert exchange symbol (BTCUSDT) to backend format (BTC_USDT) */
export function toBackendSymbol(exchangeSymbol: string): string {
  const match = exchangeSymbol.match(/^(.+)(USDT|BUSD|USDC)$/);
  if (!match) return exchangeSymbol;
  return `${match[1]}_${match[2]}`;
}
