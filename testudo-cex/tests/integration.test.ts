/**
 * Integration tests — CEX-08.
 *
 * Tests the full sidecar server end-to-end:
 * - Real HTTP server + WebSocket server
 * - Mock exchange gateway (no real exchange credentials)
 * - Full request/response cycle through Express routes
 * - WebSocket subscription → fill streaming → order_update events
 * - Reconciler integration (orphan detection → cancellation → WS event)
 */

import { describe, it, expect, mock, beforeAll, afterAll } from "bun:test";
import { WebSocket } from "ws";
import type { ExchangeGateway } from "../src/gateway";
import type { OrderUpdatePayload } from "../src/ws-fills";
import { Reconciler } from "../src/reconciler";

// ── Mock exchange factory ──

function createMockExchange(storeOverrides?: any) {
  const listeners = new Map<string, Function[]>();

  return {
    store: {
      balance: { total: 10000, free: 8000, used: 2000, upnl: 0 },
      markets: [
        {
          id: "btcusdt",
          symbol: "BTCUSDT",
          base: "BTC",
          quote: "USDT",
          active: true,
          precision: { amount: 8, price: 2 },
          limits: {
            amount: { min: 0.001, max: 1000 },
            leverage: { min: 1, max: 125 },
          },
        },
      ],
      orders: [] as any[],
      positions: [] as any[],
      loaded: {
        balance: true,
        orders: true,
        markets: true,
        tickers: true,
        positions: true,
      },
      ...storeOverrides,
    },
    placeOrder: mock(() => Promise.resolve(["entry-1", "sl-1", "tp-1"])),
    updateOrder: mock(() => Promise.resolve()),
    cancelOrders: mock(() => Promise.resolve()),
    cancelSymbolOrders: mock(() => Promise.resolve()),
    setLeverage: mock(() => Promise.resolve()),
    on: mock((event: string, cb: Function) => {
      const arr = listeners.get(event) || [];
      arr.push(cb);
      listeners.set(event, arr);
    }),
    off: mock((event: string, cb: Function) => {
      const arr = listeners.get(event) || [];
      listeners.set(
        event,
        arr.filter((f) => f !== cb)
      );
    }),
    start: mock(() => Promise.resolve()),
    dispose: mock(() => {}),
    _listeners: listeners,
    _emit(event: string, ...args: any[]) {
      (listeners.get(event) || []).forEach((cb) => cb(...args));
    },
  };
}

// ── Server setup ──

const TEST_PORT = 3198;
const BASE_URL = `http://127.0.0.1:${TEST_PORT}`;
const WS_URL = `ws://127.0.0.1:${TEST_PORT}/ws/orders`;

let mockExchange: ReturnType<typeof createMockExchange>;
let server: any;
let wss: any;
let gateway: any;

const testEnvelope = {
  exchange_id: "woo",
  credentials: { apiKey: "test-key", secret: "test-secret" },
  sandbox: true,
  params: {},
};

// Module-level mock for safe-cex
mock.module("safe-cex", () => ({
  createExchange: () => mockExchange,
}));

beforeAll(async () => {
  mockExchange = createMockExchange();

  // Import after mock setup
  const express = (await import("express")).default;
  const { createServer } = await import("http");
  const { WebSocketServer } = await import("ws");
  const { ExchangeGateway } = await import("../src/gateway");
  const { createHandlers } = await import("../src/handlers");
  const { setupFillStreaming } = await import("../src/ws-fills");

  gateway = new ExchangeGateway();
  const handlers = createHandlers(gateway);

  const app = express();
  app.use(express.json());

  app.get("/health", handlers.handleHealth);
  app.post("/balance", handlers.handleBalance);
  app.post("/order", handlers.handleOrder);
  app.post("/order/edit", handlers.handleEditOrder);
  app.post("/order/cancel", handlers.handleCancelOrder);
  app.post("/orders/cancel-all", handlers.handleCancelAllOrders);
  app.post("/orders/open", handlers.handleOpenOrders);
  app.post("/position", handlers.handlePosition);
  app.post("/leverage", handlers.handleLeverage);

  const httpServer = createServer(app);
  wss = new WebSocketServer({ server: httpServer, path: "/ws/orders" });
  setupFillStreaming(wss, gateway);

  await new Promise<void>((resolve) => {
    httpServer.listen(TEST_PORT, resolve);
  });
  server = httpServer;
});

afterAll(() => {
  wss?.close();
  server?.close();
  gateway?.disposeAll?.();
});

// ── Helper ──

async function post(path: string, body: any): Promise<Response> {
  return fetch(`${BASE_URL}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
}

function connectWs(): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(WS_URL);
    ws.on("open", () => resolve(ws));
    ws.on("error", reject);
  });
}

function waitForMessage(ws: WebSocket, timeout = 2000): Promise<any> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error("WS message timeout")),
      timeout
    );
    ws.once("message", (data: any) => {
      clearTimeout(timer);
      resolve(JSON.parse(String(data)));
    });
  });
}

/**
 * Create a message collector that eagerly buffers all WS messages.
 * Use waitFor() to wait for a message matching a predicate.
 */
function createCollector(ws: WebSocket) {
  const messages: any[] = [];
  ws.on("message", (data: any) => {
    messages.push(JSON.parse(String(data)));
  });

  return {
    messages,
    /** Wait until a message matching predicate appears (or timeout). */
    waitFor(
      predicate: (msg: any) => boolean,
      timeout = 2000
    ): Promise<any> {
      return new Promise((resolve, reject) => {
        // Check already-buffered messages first
        const existing = messages.find(predicate);
        if (existing) return resolve(existing);

        const timer = setTimeout(
          () => reject(new Error("WS collector timeout")),
          timeout
        );
        const interval = setInterval(() => {
          const found = messages.find(predicate);
          if (found) {
            clearTimeout(timer);
            clearInterval(interval);
            resolve(found);
          }
        }, 5);
      });
    },
    /** Wait for N messages matching predicate. */
    async waitForCount(
      predicate: (msg: any) => boolean,
      count: number,
      timeout = 2000
    ): Promise<any[]> {
      return new Promise((resolve, reject) => {
        const timer = setTimeout(
          () => reject(new Error(`WS collector timeout: wanted ${count}, got ${messages.filter(predicate).length}`)),
          timeout
        );
        const interval = setInterval(() => {
          const found = messages.filter(predicate);
          if (found.length >= count) {
            clearTimeout(timer);
            clearInterval(interval);
            resolve(found.slice(0, count));
          }
        }, 5);
      });
    },
  };
}

// ── Phase 1: Health Check ──

describe("Phase 1: Build Verification", () => {
  it("GET /health returns {ok: true}", async () => {
    const res = await fetch(`${BASE_URL}/health`);
    const body = await res.json();
    expect(res.status).toBe(200);
    expect(body).toEqual({ ok: true });
  });
});

// ── Phase 2: HTTP API Integration ──

describe("Phase 2: HTTP API Integration", () => {
  it("POST /balance returns stringified balance array", async () => {
    const res = await post("/balance", { ...testEnvelope });
    const body = await res.json();

    expect(res.status).toBe(200);
    expect(body).toEqual([
      { asset: "USDT", total: "10000", free: "8000", used: "2000" },
    ]);
  });

  it("POST /order places bracket order and returns all three IDs", async () => {
    const res = await post("/order", {
      ...testEnvelope,
      params: {
        symbol: "BTC_USDT",
        type: "limit",
        side: "buy",
        amount: "0.001",
        price: "70000",
        clientOrderId: "testudo:g1:entry",
        stopLoss: { triggerPrice: "69000" },
        takeProfit: { triggerPrice: "72000" },
      },
    });
    const body = await res.json();

    expect(res.status).toBe(200);
    expect(body.id).toBe("entry-1");
    expect(body.stopLossOrderId).toBe("sl-1");
    expect(body.takeProfitOrderId).toBe("tp-1");
    expect(body.clientOrderId).toBe("testudo:g1:entry");
    expect(body.symbol).toBe("BTC_USDT");
    expect(typeof body.amount).toBe("string");
    expect(typeof body.price).toBe("string");
  });

  it("POST /position returns positions from Store", async () => {
    mockExchange.store.positions = [
      {
        symbol: "BTCUSDT",
        side: "long",
        contracts: 0.001,
        entryPrice: 70000,
        unrealizedPnl: 5,
      },
    ];

    const res = await post("/position", { ...testEnvelope });
    const body = await res.json();

    expect(res.status).toBe(200);
    expect(body).toHaveLength(1);
    expect(body[0].symbol).toBe("BTC_USDT");
    expect(body[0].contracts).toBe("0.001");
    expect(body[0].side).toBe("long");
  });

  it("POST /orders/open returns open orders from Store", async () => {
    mockExchange.store.orders = [
      {
        id: "entry-1",
        symbol: "BTCUSDT",
        type: "limit",
        side: "buy",
        price: 70000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
        status: "open",
        reduceOnly: false,
      },
      {
        id: "sl-1",
        symbol: "BTCUSDT",
        type: "stop_market",
        side: "sell",
        price: 69000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
        status: "open",
        reduceOnly: true,
      },
    ];

    const res = await post("/orders/open", {
      ...testEnvelope,
      params: { symbol: "BTC_USDT" },
    });
    const body = await res.json();

    expect(res.status).toBe(200);
    expect(body).toHaveLength(2);
    expect(body[0].id).toBe("entry-1");
    expect(body[1].id).toBe("sl-1");
  });

  it("POST /order/cancel returns success", async () => {
    const res = await post("/order/cancel", {
      ...testEnvelope,
      params: { orderId: "entry-1", symbol: "BTC_USDT" },
    });
    const body = await res.json();

    expect(res.status).toBe(200);
    expect(body).toEqual({ success: true });
  });

  it("POST /orders/cancel-all returns success", async () => {
    const res = await post("/orders/cancel-all", {
      ...testEnvelope,
      params: { symbol: "BTC_USDT" },
    });
    const body = await res.json();

    expect(res.status).toBe(200);
    expect(body.success).toBe(true);
  });

  it("POST /leverage sets leverage and returns success", async () => {
    const res = await post("/leverage", {
      ...testEnvelope,
      params: { symbol: "BTC_USDT", leverage: 10 },
    });
    const body = await res.json();

    expect(res.status).toBe(200);
    expect(body).toEqual({ success: true });
  });

  it("returns 401 for auth errors", async () => {
    // Temporarily make gateway fail with auth error
    const origGetOrCreate = gateway.getOrCreate.bind(gateway);
    gateway.getOrCreate = () =>
      Promise.reject(new Error("401 Authentication failed"));

    const res = await post("/balance", {
      exchange_id: "woo",
      credentials: { apiKey: "bad-key", secret: "bad-secret" },
      sandbox: true,
    });
    const body = await res.json();

    expect(res.status).toBe(401);
    expect(body.code).toBe("AuthenticationError");

    // Restore
    gateway.getOrCreate = origGetOrCreate;
  });
});

// ── Phase 3: WebSocket Fill Streaming ──

describe("Phase 3: WebSocket Fill Streaming", () => {
  it("accepts connection and responds to subscribe", async () => {
    const ws = await connectWs();

    ws.send(
      JSON.stringify({
        action: "subscribe",
        exchange_id: "woo",
        credentials: { apiKey: "test-key", secret: "test-secret" },
        sandbox: true,
        symbols: ["BTC_USDT"],
      })
    );

    const msg = await waitForMessage(ws);
    expect(msg.event).toBe("subscribed");
    expect(msg.message).toBe("Streaming order updates");

    ws.close();
  });

  it("forwards fill events as order_update with status=closed", async () => {
    // Set up store with a known order
    mockExchange.store.orders = [
      {
        id: "entry-100",
        symbol: "BTCUSDT",
        side: "buy",
        price: 70000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
      },
    ];

    const ws = await connectWs();
    const collector = createCollector(ws);

    ws.send(
      JSON.stringify({
        action: "subscribe",
        exchange_id: "woo",
        credentials: { apiKey: "test-key", secret: "test-secret" },
        sandbox: true,
        symbols: ["BTC_USDT"],
      })
    );

    // Wait for subscribed confirmation
    await collector.waitFor((m) => m.event === "subscribed");

    // Simulate fill + order removal (as safe-cex would emit)
    mockExchange._emit("fill", {
      symbol: "BTCUSDT",
      side: "buy",
      price: 70000,
      amount: 0.001,
    });
    mockExchange.store.orders = [];
    mockExchange._emit("update", { orders: [] });

    // Should receive an order_update
    const msg = await collector.waitFor((m) => m.event === "order_update");
    expect(msg.data.id).toBe("entry-100");
    expect(msg.data.status).toBe("closed");
    expect(msg.data.symbol).toBe("BTC_USDT");
    expect(msg.data.side).toBe("buy");
    // FIX-09 FR-1: filled omitted; economics are REST-derived via FillReconciler
    expect(msg.data.filled).toBeUndefined();
    expect(typeof msg.data.timestamp).toBe("number");

    ws.close();
  });

  it("detects order cancellation via Store diff", async () => {
    // Set up store with SL + TP orders
    mockExchange.store.orders = [
      {
        id: "sl-100",
        symbol: "BTCUSDT",
        side: "sell",
        price: 69000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
      },
      {
        id: "tp-100",
        symbol: "BTCUSDT",
        side: "sell",
        price: 72000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
      },
    ];

    const ws = await connectWs();
    const collector = createCollector(ws);

    ws.send(
      JSON.stringify({
        action: "subscribe",
        exchange_id: "woo",
        credentials: { apiKey: "test-key", secret: "test-secret" },
        sandbox: true,
        symbols: ["BTC_USDT"],
      })
    );

    await collector.waitFor((m) => m.event === "subscribed");

    // Simulate: SL fills, TP gets cancelled (OCO behavior)
    mockExchange._emit("fill", {
      symbol: "BTCUSDT",
      side: "sell",
      price: 69000,
      amount: 0.001,
    });
    mockExchange.store.orders = [];
    mockExchange._emit("update", { orders: [] });

    // Should receive 2 order_update events
    const updates = await collector.waitForCount(
      (m) => m.event === "order_update",
      2
    );

    const statuses = updates.map((m: any) => m.data.status).sort();
    expect(statuses).toEqual(["canceled", "closed"]);

    ws.close();
  });
});

// ── Phase 4: Reconciler Integration ──

describe("Phase 4: Reconciler Integration", () => {
  it("detects orphaned SL/TP orders when position is closed", async () => {
    // Simulate: position closed but SL/TP orders remain (WebSocket event missed)
    const exchange = createMockExchange({
      positions: [], // No position
      orders: [
        {
          id: "sl-orphan",
          symbol: "BTCUSDT",
          type: "stop_market",
          side: "sell",
          price: 69000,
          amount: 0.001,
          reduceOnly: true,
        },
        {
          id: "tp-orphan",
          symbol: "BTCUSDT",
          type: "take_profit_market",
          side: "sell",
          price: 72000,
          amount: 0.001,
          reduceOnly: true,
        },
      ],
    });

    const canceled: string[] = [];
    const reconciler = new Reconciler();
    await reconciler.reconcileOnce(exchange as any, (orderId, symbol) => {
      canceled.push(orderId);
    });

    expect(canceled).toContain("sl-orphan");
    expect(canceled).toContain("tp-orphan");
    expect(exchange.cancelSymbolOrders).toHaveBeenCalledWith("BTCUSDT");
  });

  it("preserves entry orders (non-reduce-only)", async () => {
    const exchange = createMockExchange({
      positions: [],
      orders: [
        {
          id: "entry-pending",
          symbol: "BTCUSDT",
          type: "limit",
          side: "buy",
          price: 70000,
          amount: 0.001,
          reduceOnly: false,
        },
      ],
    });

    const canceled: string[] = [];
    const reconciler = new Reconciler();
    await reconciler.reconcileOnce(exchange as any, (orderId) => {
      canceled.push(orderId);
    });

    expect(canceled).toHaveLength(0);
    expect(exchange.cancelSymbolOrders).not.toHaveBeenCalled();
  });

  it("skips symbols with active positions", async () => {
    const exchange = createMockExchange({
      positions: [{ symbol: "BTCUSDT", contracts: 0.001, side: "long" }],
      orders: [
        {
          id: "sl-valid",
          symbol: "BTCUSDT",
          type: "stop_market",
          side: "sell",
          price: 69000,
          amount: 0.001,
          reduceOnly: true,
        },
      ],
    });

    const canceled: string[] = [];
    const reconciler = new Reconciler();
    await reconciler.reconcileOnce(exchange as any, (orderId) => {
      canceled.push(orderId);
    });

    expect(canceled).toHaveLength(0);
    expect(exchange.cancelSymbolOrders).not.toHaveBeenCalled();
  });

  it("full pipeline: reconciler → orphan cancel → WS event", async () => {
    // Set up store with orphaned orders (no position)
    mockExchange.store.positions = [];
    mockExchange.store.orders = [
      {
        id: "sl-200",
        symbol: "BTCUSDT",
        type: "stop_market",
        side: "sell",
        price: 69000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
        reduceOnly: true,
      },
    ];

    const ws = await connectWs();
    const collector = createCollector(ws);

    ws.send(
      JSON.stringify({
        action: "subscribe",
        exchange_id: "woo",
        credentials: { apiKey: "test-key", secret: "test-secret" },
        sandbox: true,
        symbols: ["BTC_USDT"],
      })
    );

    await collector.waitFor((m) => m.event === "subscribed");

    // Run reconciler — it should cancel the orphan
    const reconciler = new Reconciler();
    const canceled: string[] = [];

    // Make cancelSymbolOrders actually remove orders from store + trigger update
    mockExchange.cancelSymbolOrders = mock(async () => {
      mockExchange.store.orders = [];
      // safe-cex would emit an update event after cancellation
      mockExchange._emit("update", { orders: [] });
    });

    await reconciler.reconcileOnce(mockExchange as any, (orderId, symbol) => {
      canceled.push(orderId);
    });

    expect(canceled).toContain("sl-200");

    // The WS client should receive the cancellation event
    const msg = await collector.waitFor((m) => m.event === "order_update");
    expect(msg.data.id).toBe("sl-200");
    expect(msg.data.status).toBe("canceled");

    ws.close();
  });
});

// ── Phase 5: Full Bracket Order Lifecycle ──

describe("Phase 5: Full Bracket Order Lifecycle (mock)", () => {
  it("place bracket → entry fill → SL fill → TP cancelled → no orphans", async () => {
    // Reset store
    mockExchange.store.orders = [];
    mockExchange.store.positions = [];

    const ws = await connectWs();
    const collector = createCollector(ws);

    ws.send(
      JSON.stringify({
        action: "subscribe",
        exchange_id: "woo",
        credentials: { apiKey: "test-key", secret: "test-secret" },
        sandbox: true,
        symbols: ["BTC_USDT"],
      })
    );

    await collector.waitFor((m) => m.event === "subscribed");

    // Step 1: Place bracket order via HTTP
    const orderRes = await post("/order", {
      ...testEnvelope,
      params: {
        symbol: "BTC_USDT",
        type: "limit",
        side: "buy",
        amount: "0.001",
        price: "70000",
        stopLoss: { triggerPrice: "69000" },
        takeProfit: { triggerPrice: "72000" },
      },
    });
    const orderBody = await orderRes.json();
    expect(orderBody.id).toBe("entry-1");
    expect(orderBody.stopLossOrderId).toBe("sl-1");
    expect(orderBody.takeProfitOrderId).toBe("tp-1");

    // Step 2: Simulate exchange state after bracket placement
    mockExchange.store.orders = [
      {
        id: "entry-1",
        symbol: "BTCUSDT",
        side: "buy",
        price: 70000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
      },
      {
        id: "sl-1",
        symbol: "BTCUSDT",
        side: "sell",
        price: 69000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
        reduceOnly: true,
        type: "stop_market",
      },
      {
        id: "tp-1",
        symbol: "BTCUSDT",
        side: "sell",
        price: 72000,
        amount: 0.001,
        filled: 0,
        remaining: 0.001,
        reduceOnly: true,
        type: "take_profit_market",
      },
    ];
    // Trigger an update so the WS subscriber knows about these orders
    mockExchange._emit("update", { orders: mockExchange.store.orders });

    // Step 3: Entry fills
    mockExchange._emit("fill", {
      symbol: "BTCUSDT",
      side: "buy",
      price: 70000,
      amount: 0.001,
    });
    mockExchange.store.orders = [
      mockExchange.store.orders[1], // sl-1
      mockExchange.store.orders[2], // tp-1
    ];
    mockExchange.store.positions = [
      { symbol: "BTCUSDT", contracts: 0.001, side: "long" },
    ];
    mockExchange._emit("update", { orders: mockExchange.store.orders });

    const entryFill = await collector.waitFor(
      (m) => m.event === "order_update" && m.data.id === "entry-1"
    );
    expect(entryFill.data.status).toBe("closed");

    // Step 4: SL triggers — position closes, TP gets cancelled (OCO)
    mockExchange._emit("fill", {
      symbol: "BTCUSDT",
      side: "sell",
      price: 69000,
      amount: 0.001,
    });
    mockExchange.store.orders = [];
    mockExchange.store.positions = [];
    mockExchange._emit("update", { orders: [] });

    // Should get SL filled + TP cancelled (2 more order_update events)
    const ocoUpdates = await collector.waitForCount(
      (m) =>
        m.event === "order_update" &&
        (m.data.id === "sl-1" || m.data.id === "tp-1"),
      2
    );

    const statuses = new Map(
      ocoUpdates.map((m: any) => [m.data.id, m.data.status])
    );

    // SL should be "closed" (filled), TP should be "canceled" (OCO)
    expect(statuses.get("sl-1")).toBe("closed");
    expect(statuses.get("tp-1")).toBe("canceled");

    // Step 5: Verify no orphaned orders remain
    const reconciler = new Reconciler();
    const orphans: string[] = [];
    await reconciler.reconcileOnce(mockExchange as any, (id) =>
      orphans.push(id)
    );
    expect(orphans).toHaveLength(0);

    // Step 6: Verify via HTTP that no orders remain
    const openRes = await post("/orders/open", {
      ...testEnvelope,
      params: { symbol: "BTC_USDT" },
    });
    const openBody = await openRes.json();
    expect(openBody).toHaveLength(0);

    // Verify no positions remain
    const posRes = await post("/position", { ...testEnvelope, params: {} });
    const posBody = await posRes.json();
    expect(posBody).toHaveLength(0);

    ws.close();
  });
});
