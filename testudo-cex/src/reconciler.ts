/**
 * Polling reconciler — CEX-06.
 *
 * Defense-in-depth safety net that periodically checks exchange Store state
 * for orphaned orders (reduce-only / stop / TP orders that exist without a
 * corresponding position). Cancels orphans and emits synthetic events so the
 * Rust backend updates its state.
 */

import type { BaseExchange } from "safe-cex/dist/exchanges/base";

export type OnOrphanCanceled = (orderId: string, symbol: string) => void;

export class Reconciler {
  private interval: ReturnType<typeof setInterval> | null = null;

  get isRunning(): boolean {
    return this.interval !== null;
  }

  /**
   * Start the polling loop.
   * @param exchange  safe-cex exchange instance (reads store, calls cancelSymbolOrders)
   * @param onOrphanCanceled  callback for each orphaned order cancelled
   * @param intervalMs  polling interval in ms (default 15s)
   */
  start(
    exchange: BaseExchange,
    onOrphanCanceled: OnOrphanCanceled,
    intervalMs: number = 15_000
  ): void {
    this.interval = setInterval(async () => {
      await this.reconcileOnce(exchange, onOrphanCanceled);
    }, intervalMs);
  }

  /** Stop the polling loop. Safe to call when not running. */
  stop(): void {
    if (this.interval) {
      clearInterval(this.interval);
      this.interval = null;
    }
  }

  /**
   * Run a single reconciliation pass. Exposed for testing.
   *
   * Reads positions and orders from exchange.store (no API calls).
   * Detects orphaned orders: reduce-only / stop_market / take_profit_market
   * orders that exist for a symbol with no active position.
   */
  async reconcileOnce(
    exchange: Pick<BaseExchange, "store" | "cancelSymbolOrders">,
    onOrphanCanceled: OnOrphanCanceled
  ): Promise<void> {
    const { positions, orders } = exchange.store;

    // Group orders by symbol
    const ordersBySymbol = new Map<string, typeof orders>();
    for (const order of orders) {
      const existing = ordersBySymbol.get(order.symbol) || [];
      existing.push(order);
      ordersBySymbol.set(order.symbol, existing);
    }

    for (const [symbol, symbolOrders] of ordersBySymbol) {
      const hasPosition = positions.some(
        (p) => p.symbol === symbol && Math.abs(p.contracts) > 0
      );

      if (hasPosition) continue;

      // Only cancel reduce-only / stop / TP orders — entry orders are expected
      const orphans = symbolOrders.filter(
        (o) =>
          o.reduceOnly ||
          o.type === "stop_market" ||
          o.type === "take_profit_market"
      );

      if (orphans.length === 0) continue;

      console.warn(
        `[reconciler] Orphaned orders detected for ${symbol}: ${orphans.map((o) => o.id).join(", ")}`
      );

      try {
        await exchange.cancelSymbolOrders(symbol);
        for (const order of orphans) {
          onOrphanCanceled(order.id, symbol);
        }
      } catch (err) {
        console.error(
          `[reconciler] Failed to cancel orphans for ${symbol}:`,
          err
        );
      }
    }
  }
}
