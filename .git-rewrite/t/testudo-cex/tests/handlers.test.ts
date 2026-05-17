import { describe, it, expect, mock, beforeEach } from "bun:test";
import {
  stringify,
  parseEnvelope,
  mapError,
  createHandlers,
} from "../src/handlers";
import { ExchangeGateway } from "../src/gateway";

// ── Mock exchange with Store ──

function mockExchange(storeData?: any) {
  return {
    store: {
      balance: { total: 10000, free: 8000, used: 2000, upnl: 150 },
      markets: [
        { id: "btcusdt", symbol: "BTCUSDT", base: "BTC", quote: "USDT", active: true, precision: { amount: 8, price: 2 }, limits: { amount: { min: 0.001, max: 1000 }, leverage: { min: 1, max: 125 } } },
        { id: "ethusdt", symbol: "ETHUSDT", base: "ETH", quote: "USDT", active: true, precision: { amount: 8, price: 2 }, limits: { amount: { min: 0.001, max: 1000 }, leverage: { min: 1, max: 125 } } },
      ],
      orders: [
        {
          id: "order-1",
          symbol: "BTCUSDT",
          type: "limit",
          side: "buy",
          price: 70000,
          amount: 0.01,
          filled: 0,
          remaining: 0.01,
          status: "open",
          reduceOnly: false,
        },
        {
          id: "order-2",
          symbol: "ETHUSDT",
          type: "limit",
          side: "sell",
          price: 3500,
          amount: 0.5,
          filled: 0.1,
          remaining: 0.4,
          status: "open",
          reduceOnly: false,
        },
      ],
      positions: [
        {
          symbol: "BTCUSDT",
          side: "long",
          contracts: 0.05,
          entryPrice: 69000,
          unrealizedPnl: 50,
          leverage: 10,
          notional: 3450,
          liquidationPrice: 62000,
        },
      ],
      loaded: { balance: true, orders: true, markets: true, tickers: true, positions: true },
      ...storeData,
    },
    placeOrder: mock(() => Promise.resolve(["entry-123"])),
    updateOrder: mock(() => Promise.resolve()),
    cancelOrders: mock(() => Promise.resolve()),
    cancelSymbolOrders: mock(() => Promise.resolve()),
    setLeverage: mock(() => Promise.resolve()),
    on: mock(() => {}),
    start: mock(() => Promise.resolve()),
    dispose: mock(() => {}),
  };
}

// ── Mock req/res ──

function mockReq(body: any = {}): any {
  return { body };
}

function mockRes(): any {
  const res: any = {
    _status: 200,
    _json: null,
    status(code: number) {
      res._status = code;
      return res;
    },
    json(data: any) {
      res._json = data;
      return res;
    },
  };
  return res;
}

const testEnvelope = {
  exchange_id: "woo",
  credentials: { apiKey: "test-key", secret: "test-secret" },
  sandbox: false,
};

// ── Helper tests ──

describe("stringify", () => {
  it("converts numbers to strings", () => {
    expect(stringify(42)).toBe("42");
    expect(stringify(0.001)).toBe("0.001");
    expect(stringify(0)).toBe("0");
  });

  it("passes through strings", () => {
    expect(stringify("100")).toBe("100");
  });

  it("returns null for nullish values", () => {
    expect(stringify(null)).toBeNull();
    expect(stringify(undefined)).toBeNull();
  });
});

describe("parseEnvelope", () => {
  it("parses valid envelope", () => {
    const result = parseEnvelope({
      exchange_id: "woo",
      credentials: { apiKey: "key", secret: "sec", password: "pass" },
      sandbox: true,
      params: { symbol: "BTCUSDT" },
    });

    expect(result.exchangeId).toBe("woo");
    expect(result.credentials.key).toBe("key");
    expect(result.credentials.secret).toBe("sec");
    expect(result.credentials.passphrase).toBe("pass");
    expect(result.sandbox).toBe(true);
    expect(result.params.symbol).toBe("BTCUSDT");
  });

  it("throws on missing exchange_id", () => {
    expect(() =>
      parseEnvelope({ credentials: { apiKey: "k", secret: "s" } })
    ).toThrow("Missing exchange_id");
  });

  it("throws on missing credentials", () => {
    expect(() =>
      parseEnvelope({ exchange_id: "woo", credentials: {} })
    ).toThrow("Missing or incomplete credentials");
  });

  it("defaults params to empty object", () => {
    const result = parseEnvelope({
      exchange_id: "woo",
      credentials: { apiKey: "k", secret: "s" },
    });
    expect(result.params).toEqual({});
  });

  it("maps applicationId from credentials", () => {
    const result = parseEnvelope({
      exchange_id: "woo",
      credentials: { apiKey: "k", secret: "s", applicationId: "app-123" },
    });
    expect(result.credentials.applicationId).toBe("app-123");
  });
});

describe("mapError", () => {
  it("maps auth errors to 401", () => {
    const result = mapError(new Error("Authentication failed"));
    expect(result.status).toBe(401);
    expect(result.body.code).toBe("AuthenticationError");
  });

  it("maps insufficient funds to 402", () => {
    const result = mapError(new Error("insufficient margin"));
    expect(result.status).toBe(402);
    expect(result.body.code).toBe("InsufficientFunds");
  });

  it("maps not found to 404", () => {
    const result = mapError(new Error("Order not found"));
    expect(result.status).toBe(404);
    expect(result.body.code).toBe("OrderNotFound");
  });

  it("maps rate limit to 429", () => {
    const result = mapError(new Error("rate limit exceeded"));
    expect(result.status).toBe(429);
    expect(result.body.code).toBe("RateLimitExceeded");
  });

  it("defaults to 502 for unknown errors", () => {
    const result = mapError(new Error("something broke"));
    expect(result.status).toBe(502);
    expect(result.body.code).toBe("ExchangeError");
  });

  it("handles non-Error objects", () => {
    const result = mapError("string error");
    expect(result.status).toBe(502);
    expect(result.body.error).toBe("string error");
  });
});

// ── Handler tests ──

describe("handlers", () => {
  let exchange: ReturnType<typeof mockExchange>;
  let gateway: ExchangeGateway;
  let handlers: ReturnType<typeof createHandlers>;

  beforeEach(() => {
    exchange = mockExchange();
    // Create a gateway and mock getOrCreate to return our mock exchange
    gateway = new ExchangeGateway();
    (gateway as any).getOrCreate = mock(() => Promise.resolve(exchange));
    handlers = createHandlers(gateway);
  });

  describe("GET /health", () => {
    it("returns ok:true", async () => {
      const req = mockReq();
      const res = mockRes();
      await handlers.handleHealth(req, res);
      expect(res._json).toEqual({ ok: true });
    });
  });

  describe("POST /balance", () => {
    it("returns balance from Store as string array", async () => {
      const req = mockReq({ ...testEnvelope });
      const res = mockRes();
      await handlers.handleBalance(req, res);

      expect(res._status).toBe(200);
      expect(res._json).toEqual([
        { asset: "USDT", total: "10000", free: "8000", used: "2000" },
      ]);
    });

    it("returns error on gateway failure", async () => {
      (gateway as any).getOrCreate = mock(() =>
        Promise.reject(new Error("Authentication failed"))
      );
      handlers = createHandlers(gateway);

      const req = mockReq({ ...testEnvelope });
      const res = mockRes();
      await handlers.handleBalance(req, res);

      expect(res._status).toBe(401);
      expect(res._json.code).toBe("AuthenticationError");
    });
  });

  describe("POST /order", () => {
    it("places order and returns SidecarOrderResponse shape", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: {
          symbol: "BTC_USDT",
          type: "limit",
          side: "buy",
          amount: "0.01",
          price: "70000",
          clientOrderId: "testudo:g1:entry",
        },
      });
      const res = mockRes();
      await handlers.handleOrder(req, res);

      expect(res._status).toBe(200);
      expect(res._json.id).toBe("entry-123");
      expect(res._json.clientOrderId).toBe("testudo:g1:entry");
      expect(res._json.status).toBe("open");
      expect(res._json.symbol).toBe("BTC_USDT");
      expect(res._json.side).toBe("buy");
      expect(res._json.amount).toBe("0.01");
      expect(res._json.price).toBe("70000");
      expect(res._json.filled).toBe("0");
      expect(res._json.remaining).toBe("0.01");
      expect(res._json.stopLossOrderId).toBeNull();
      expect(res._json.takeProfitOrderId).toBeNull();
    });

    it("passes bracket order SL/TP to placeOrder", async () => {
      exchange.placeOrder = mock(() =>
        Promise.resolve(["entry-1", "sl-1", "tp-1"])
      );

      const req = mockReq({
        ...testEnvelope,
        params: {
          symbol: "BTC_USDT",
          type: "limit",
          side: "buy",
          amount: "0.01",
          price: "70000",
          stopLoss: { triggerPrice: "69000" },
          takeProfit: { triggerPrice: "72000" },
        },
      });
      const res = mockRes();
      await handlers.handleOrder(req, res);

      // Verify placeOrder was called with SL/TP
      const opts = exchange.placeOrder.mock.calls[0][0];
      expect(opts.stopLoss).toBe(69000);
      expect(opts.takeProfit).toBe(72000);

      // Verify response includes bracket IDs
      expect(res._json.id).toBe("entry-1");
      expect(res._json.stopLossOrderId).toBe("sl-1");
      expect(res._json.takeProfitOrderId).toBe("tp-1");
    });

    it("sets leverage before placing order (FR-16)", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: {
          symbol: "BTC_USDT",
          type: "limit",
          side: "buy",
          amount: "0.01",
          price: "70000",
          leverage: 10,
        },
      });
      const res = mockRes();
      await handlers.handleOrder(req, res);

      expect(exchange.setLeverage).toHaveBeenCalledWith("BTCUSDT", 10);
    });

    it("continues if leverage setting fails", async () => {
      exchange.setLeverage = mock(() =>
        Promise.reject(new Error("leverage not supported"))
      );

      const req = mockReq({
        ...testEnvelope,
        params: {
          symbol: "BTC_USDT",
          type: "limit",
          side: "buy",
          amount: "0.01",
          price: "70000",
          leverage: 10,
        },
      });
      const res = mockRes();
      await handlers.handleOrder(req, res);

      expect(res._status).toBe(200);
      expect(res._json.id).toBe("entry-123");
    });

    it("passes reduceOnly to placeOrder (FR-15)", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: {
          symbol: "BTC_USDT",
          type: "limit",
          side: "sell",
          amount: "0.01",
          price: "71000",
          reduceOnly: true,
        },
      });
      const res = mockRes();
      await handlers.handleOrder(req, res);

      const opts = exchange.placeOrder.mock.calls[0][0];
      expect(opts.reduceOnly).toBe(true);
    });
  });

  describe("POST /order/edit", () => {
    it("finds order in store and updates it", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: {
          orderId: "order-1",
          symbol: "BTC_USDT",
          type: "limit",
          side: "buy",
          price: "69500",
        },
      });
      const res = mockRes();
      await handlers.handleEditOrder(req, res);

      expect(res._status).toBe(200);
      expect(res._json.id).toBe("order-1");
      expect(res._json.price).toBe("69500");
      expect(exchange.updateOrder).toHaveBeenCalledTimes(1);

      const updateArgs = exchange.updateOrder.mock.calls[0][0];
      expect(updateArgs.order.id).toBe("order-1");
      expect(updateArgs.update.price).toBe(69500);
    });

    it("returns 404 if order not in store", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: {
          orderId: "nonexistent",
          symbol: "BTC_USDT",
          type: "limit",
          side: "buy",
          price: "69500",
        },
      });
      const res = mockRes();
      await handlers.handleEditOrder(req, res);

      expect(res._status).toBe(404);
      expect(res._json.code).toBe("OrderNotFound");
    });
  });

  describe("POST /order/cancel", () => {
    it("finds order in store and cancels it", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: { orderId: "order-1", symbol: "BTC_USDT" },
      });
      const res = mockRes();
      await handlers.handleCancelOrder(req, res);

      expect(res._status).toBe(200);
      expect(res._json).toEqual({ success: true });
      expect(exchange.cancelOrders).toHaveBeenCalledTimes(1);

      const cancelledOrders = exchange.cancelOrders.mock.calls[0][0];
      expect(cancelledOrders[0].id).toBe("order-1");
    });

    it("passes minimal object if order not in store", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: { orderId: "unknown-id", symbol: "BTC_USDT" },
      });
      const res = mockRes();
      await handlers.handleCancelOrder(req, res);

      expect(res._status).toBe(200);
      const cancelledOrders = exchange.cancelOrders.mock.calls[0][0];
      expect(cancelledOrders[0].id).toBe("unknown-id");
      expect(cancelledOrders[0].symbol).toBe("BTCUSDT"); // exchange format for safe-cex
    });
  });

  describe("POST /orders/cancel-all", () => {
    it("cancels all orders for symbol", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: { symbol: "BTC_USDT" },
      });
      const res = mockRes();
      await handlers.handleCancelAllOrders(req, res);

      expect(res._status).toBe(200);
      expect(res._json).toEqual({ success: true, cancelled: 0 });
      expect(exchange.cancelSymbolOrders).toHaveBeenCalledWith("BTCUSDT");
    });
  });

  describe("POST /position", () => {
    it("returns positions from Store as stringified values", async () => {
      const req = mockReq({ ...testEnvelope, params: {} });
      const res = mockRes();
      await handlers.handlePosition(req, res);

      expect(res._status).toBe(200);
      expect(res._json).toEqual([
        {
          symbol: "BTC_USDT",
          side: "long",
          contracts: "0.05",
          entryPrice: "69000",
          unrealizedPnl: "50",
        },
      ]);
    });

    it("filters positions by symbol", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: { symbol: "ETH_USDT" },
      });
      const res = mockRes();
      await handlers.handlePosition(req, res);

      expect(res._json).toEqual([]);
    });
  });

  describe("POST /leverage", () => {
    it("calls setLeverage with correct param order (symbol, leverage)", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: { symbol: "BTC_USDT", leverage: 10 },
      });
      const res = mockRes();
      await handlers.handleLeverage(req, res);

      expect(res._status).toBe(200);
      expect(res._json).toEqual({ success: true });
      expect(exchange.setLeverage).toHaveBeenCalledWith("BTCUSDT", 10);
    });
  });

  describe("POST /orders/open", () => {
    it("returns open orders from Store", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: { symbol: "BTC_USDT" },
      });
      const res = mockRes();
      await handlers.handleOpenOrders(req, res);

      expect(res._status).toBe(200);
      expect(res._json).toHaveLength(1);
      expect(res._json[0].id).toBe("order-1");
      expect(res._json[0].symbol).toBe("BTC_USDT");
      expect(res._json[0].price).toBe("70000");
      expect(res._json[0].amount).toBe("0.01");
    });

    it("returns all orders when no symbol filter", async () => {
      const req = mockReq({ ...testEnvelope, params: {} });
      const res = mockRes();
      await handlers.handleOpenOrders(req, res);

      expect(res._json).toHaveLength(2);
    });

    it("all numeric fields are strings (FR-10)", async () => {
      const req = mockReq({
        ...testEnvelope,
        params: { symbol: "BTC_USDT" },
      });
      const res = mockRes();
      await handlers.handleOpenOrders(req, res);

      const order = res._json[0];
      expect(typeof order.id).toBe("string");
      expect(typeof order.price).toBe("string");
      expect(typeof order.amount).toBe("string");
      expect(typeof order.filled).toBe("string");
      expect(typeof order.remaining).toBe("string");
    });
  });
});
