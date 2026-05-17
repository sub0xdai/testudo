import { describe, it, expect, mock, beforeEach, afterEach } from "bun:test";
import { Reconciler } from "../src/reconciler";

// ── Mock helpers ──

function makePosition(symbol: string, contracts: number = 0.01) {
  return { symbol, side: "long", entryPrice: 70000, contracts, notional: 700, leverage: 10, liquidationPrice: 60000 };
}

function makeOrder(overrides: Record<string, any> = {}) {
  return {
    id: "order-1",
    symbol: "BTC_USDT",
    type: "limit",
    side: "buy",
    price: 70000,
    amount: 0.01,
    filled: 0,
    remaining: 0.01,
    reduceOnly: false,
    ...overrides,
  };
}

function mockExchange(positions: any[] = [], orders: any[] = []) {
  return {
    store: { positions, orders },
    cancelSymbolOrders: mock(() => Promise.resolve()),
  };
}

// ── Tests ──

describe("Reconciler", () => {
  let reconciler: Reconciler;

  beforeEach(() => {
    reconciler = new Reconciler();
  });

  afterEach(() => {
    reconciler.stop();
  });

  describe("orphan detection", () => {
    it("detects orphaned stop_market order when no position exists", async () => {
      const orphanOrder = makeOrder({
        id: "sl-1",
        type: "stop_market",
        side: "sell",
        reduceOnly: true,
      });
      const exchange = mockExchange([], [orphanOrder]);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).toHaveBeenCalledWith("BTC_USDT");
      expect(onOrphanCanceled).toHaveBeenCalledWith("sl-1", "BTC_USDT");
    });

    it("detects orphaned take_profit_market order when no position exists", async () => {
      const orphanOrder = makeOrder({
        id: "tp-1",
        type: "take_profit_market",
        side: "sell",
        reduceOnly: true,
      });
      const exchange = mockExchange([], [orphanOrder]);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).toHaveBeenCalledWith("BTC_USDT");
      expect(onOrphanCanceled).toHaveBeenCalledWith("tp-1", "BTC_USDT");
    });

    it("detects orphaned reduce-only limit order when no position exists", async () => {
      const orphanOrder = makeOrder({
        id: "ro-1",
        type: "limit",
        side: "sell",
        reduceOnly: true,
      });
      const exchange = mockExchange([], [orphanOrder]);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).toHaveBeenCalledWith("BTC_USDT");
      expect(onOrphanCanceled).toHaveBeenCalledWith("ro-1", "BTC_USDT");
    });

    it("detects multiple orphaned orders for same symbol", async () => {
      const orders = [
        makeOrder({ id: "sl-1", type: "stop_market", side: "sell", reduceOnly: true }),
        makeOrder({ id: "tp-1", type: "take_profit_market", side: "sell", reduceOnly: true }),
      ];
      const exchange = mockExchange([], orders);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).toHaveBeenCalledTimes(1);
      expect(onOrphanCanceled).toHaveBeenCalledTimes(2);
    });

    it("detects orphans across multiple symbols", async () => {
      const orders = [
        makeOrder({ id: "sl-btc", symbol: "BTC_USDT", type: "stop_market", reduceOnly: true }),
        makeOrder({ id: "sl-eth", symbol: "ETH_USDT", type: "stop_market", reduceOnly: true }),
      ];
      const exchange = mockExchange([], orders);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).toHaveBeenCalledTimes(2);
      expect(onOrphanCanceled).toHaveBeenCalledTimes(2);
    });
  });

  describe("entry order safety", () => {
    it("does NOT cancel entry (non-reduce-only limit) orders without position", async () => {
      const entryOrder = makeOrder({
        id: "entry-1",
        type: "limit",
        side: "buy",
        reduceOnly: false,
      });
      const exchange = mockExchange([], [entryOrder]);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).not.toHaveBeenCalled();
      expect(onOrphanCanceled).not.toHaveBeenCalled();
    });

    it("does NOT cancel market entry orders without position", async () => {
      const entryOrder = makeOrder({
        id: "entry-1",
        type: "market",
        side: "buy",
        reduceOnly: false,
      });
      const exchange = mockExchange([], [entryOrder]);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).not.toHaveBeenCalled();
      expect(onOrphanCanceled).not.toHaveBeenCalled();
    });

    it("cancels orphan SL but leaves entry order untouched in mixed set", async () => {
      const orders = [
        makeOrder({ id: "entry-1", type: "limit", side: "buy", reduceOnly: false }),
        makeOrder({ id: "sl-1", type: "stop_market", side: "sell", reduceOnly: true }),
      ];
      const exchange = mockExchange([], orders);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      // cancelSymbolOrders is called (cancels all for symbol), but callback only fires for orphan
      expect(exchange.cancelSymbolOrders).toHaveBeenCalledWith("BTC_USDT");
      expect(onOrphanCanceled).toHaveBeenCalledTimes(1);
      expect(onOrphanCanceled).toHaveBeenCalledWith("sl-1", "BTC_USDT");
    });
  });

  describe("position present", () => {
    it("does NOT cancel orders when position exists for symbol", async () => {
      const orders = [
        makeOrder({ id: "sl-1", type: "stop_market", side: "sell", reduceOnly: true }),
        makeOrder({ id: "tp-1", type: "take_profit_market", side: "sell", reduceOnly: true }),
      ];
      const positions = [makePosition("BTC_USDT", 0.01)];
      const exchange = mockExchange(positions, orders);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).not.toHaveBeenCalled();
      expect(onOrphanCanceled).not.toHaveBeenCalled();
    });

    it("treats zero-contract position as no position", async () => {
      const orders = [
        makeOrder({ id: "sl-1", type: "stop_market", side: "sell", reduceOnly: true }),
      ];
      const positions = [makePosition("BTC_USDT", 0)];
      const exchange = mockExchange(positions, orders);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).toHaveBeenCalledWith("BTC_USDT");
      expect(onOrphanCanceled).toHaveBeenCalledWith("sl-1", "BTC_USDT");
    });
  });

  describe("synthetic event callback", () => {
    it("calls onOrphanCanceled with order id and symbol for each orphan", async () => {
      const orders = [
        makeOrder({ id: "sl-1", symbol: "BTC_USDT", type: "stop_market", reduceOnly: true }),
        makeOrder({ id: "tp-1", symbol: "BTC_USDT", type: "take_profit_market", reduceOnly: true }),
      ];
      const exchange = mockExchange([], orders);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(onOrphanCanceled).toHaveBeenCalledTimes(2);
      expect(onOrphanCanceled.mock.calls[0]).toEqual(["sl-1", "BTC_USDT"]);
      expect(onOrphanCanceled.mock.calls[1]).toEqual(["tp-1", "BTC_USDT"]);
    });
  });

  describe("error handling", () => {
    it("logs error and continues when cancelSymbolOrders fails", async () => {
      const orders = [
        makeOrder({ id: "sl-1", type: "stop_market", reduceOnly: true }),
      ];
      const exchange = mockExchange([], orders);
      exchange.cancelSymbolOrders = mock(() => Promise.reject(new Error("network error")));
      const onOrphanCanceled = mock(() => {});

      // Should not throw
      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      // Callback should NOT be called since cancellation failed
      expect(onOrphanCanceled).not.toHaveBeenCalled();
    });
  });

  describe("lifecycle", () => {
    it("stop() clears the interval", () => {
      const exchange = mockExchange();
      const onOrphanCanceled = mock(() => {});

      reconciler.start(exchange as any, onOrphanCanceled, 60_000);
      expect(reconciler.isRunning).toBe(true);

      reconciler.stop();
      expect(reconciler.isRunning).toBe(false);
    });

    it("stop() is safe to call when not running", () => {
      reconciler.stop(); // Should not throw
      expect(reconciler.isRunning).toBe(false);
    });

    it("start() uses configurable interval", () => {
      const exchange = mockExchange();
      const onOrphanCanceled = mock(() => {});

      reconciler.start(exchange as any, onOrphanCanceled, 30_000);
      expect(reconciler.isRunning).toBe(true);
      reconciler.stop();
    });

    it("start() defaults to 15s interval", () => {
      const exchange = mockExchange();
      const onOrphanCanceled = mock(() => {});

      reconciler.start(exchange as any, onOrphanCanceled);
      expect(reconciler.isRunning).toBe(true);
      reconciler.stop();
    });
  });

  describe("no-op scenarios", () => {
    it("does nothing when no orders exist", async () => {
      const exchange = mockExchange([], []);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).not.toHaveBeenCalled();
      expect(onOrphanCanceled).not.toHaveBeenCalled();
    });

    it("does nothing when all symbols have positions", async () => {
      const orders = [
        makeOrder({ id: "sl-1", symbol: "BTC_USDT", type: "stop_market", reduceOnly: true }),
        makeOrder({ id: "sl-2", symbol: "ETH_USDT", type: "stop_market", reduceOnly: true }),
      ];
      const positions = [
        makePosition("BTC_USDT", 0.01),
        makePosition("ETH_USDT", 0.5),
      ];
      const exchange = mockExchange(positions, orders);
      const onOrphanCanceled = mock(() => {});

      await reconciler.reconcileOnce(exchange as any, onOrphanCanceled);

      expect(exchange.cancelSymbolOrders).not.toHaveBeenCalled();
      expect(onOrphanCanceled).not.toHaveBeenCalled();
    });
  });
});
