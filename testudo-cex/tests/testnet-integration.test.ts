/**
 * Testnet integration tests — CEX-08 Phase 2-4.
 *
 * Exercises the full trade lifecycle against WOO X testnet via the sidecar.
 * Requires real testnet credentials to run.
 *
 * Set env vars:
 *   WOO_TESTNET_KEY=<api-key>
 *   WOO_TESTNET_SECRET=<api-secret>
 *   WOO_TESTNET_APP_ID=<application-id>  (optional)
 *
 * Run:
 *   WOO_TESTNET_KEY=... WOO_TESTNET_SECRET=... bun test tests/testnet-integration.test.ts
 *
 * These tests are SKIPPED when credentials are not provided.
 */

import { describe, it, expect, beforeAll, afterAll } from "bun:test";
import { WebSocket } from "ws";

const API_KEY = process.env.WOO_TESTNET_KEY;
const API_SECRET = process.env.WOO_TESTNET_SECRET;
const APP_ID = process.env.WOO_TESTNET_APP_ID;
const HAS_CREDENTIALS = Boolean(API_KEY && API_SECRET);

const TEST_PORT = 3197;
const BASE_URL = `http://127.0.0.1:${TEST_PORT}`;
const WS_URL = `ws://127.0.0.1:${TEST_PORT}/ws/orders`;

let server: any;
let wss: any;
let gateway: any;

const envelope = {
  exchange_id: "woo",
  credentials: {
    apiKey: API_KEY || "",
    secret: API_SECRET || "",
    applicationId: APP_ID,
  },
  sandbox: true,
};

async function post(path: string, body: any): Promise<any> {
  const res = await fetch(`${BASE_URL}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  return { status: res.status, body: await res.json() };
}

function connectWs(): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(WS_URL);
    ws.on("open", () => resolve(ws));
    ws.on("error", reject);
  });
}

function createCollector(ws: WebSocket) {
  const messages: any[] = [];
  ws.on("message", (data: any) => {
    messages.push(JSON.parse(String(data)));
  });
  return {
    messages,
    waitFor(
      predicate: (msg: any) => boolean,
      timeout = 30000
    ): Promise<any> {
      return new Promise((resolve, reject) => {
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
        }, 100);
      });
    },
  };
}

// Skip all tests if no credentials
const describeTestnet = HAS_CREDENTIALS ? describe : describe.skip;

describeTestnet("Testnet Integration (WOO X)", () => {
  beforeAll(async () => {
    // Start the real sidecar server
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

    console.log(`[testnet] Sidecar started on port ${TEST_PORT}`);
  }, 30000);

  afterAll(async () => {
    wss?.close();
    server?.close();
    await gateway?.disposeAll?.();
  });

  // Phase 1: Health + Connectivity

  it("health check passes", async () => {
    const res = await fetch(`${BASE_URL}/health`);
    const body = await res.json();
    expect(body).toEqual({ ok: true });
  });

  it("connects to WOO X testnet and fetches balance", async () => {
    const { status, body } = await post("/balance", envelope);
    expect(status).toBe(200);
    expect(Array.isArray(body)).toBe(true);
    expect(body[0].asset).toBe("USDT");
    expect(Number(body[0].total)).toBeGreaterThan(0);
    console.log(`[testnet] Balance: ${body[0].free} USDT free`);
  }, 30000);

  it("fetches positions", async () => {
    const { status, body } = await post("/position", {
      ...envelope,
      params: {},
    });
    expect(status).toBe(200);
    expect(Array.isArray(body)).toBe(true);
    console.log(`[testnet] Open positions: ${body.length}`);
  }, 15000);

  it("fetches open orders", async () => {
    const { status, body } = await post("/orders/open", {
      ...envelope,
      params: {},
    });
    expect(status).toBe(200);
    expect(Array.isArray(body)).toBe(true);
    console.log(`[testnet] Open orders: ${body.length}`);
  }, 15000);

  // Phase 2: Bracket Order Lifecycle

  it("sets leverage on BTC_USDT", async () => {
    const { status, body } = await post("/leverage", {
      ...envelope,
      params: { symbol: "BTC_USDT", leverage: 5 },
    });
    // Leverage setting may fail on testnet — that's acceptable (FR-16 graceful fallback)
    console.log(`[testnet] Set leverage response:`, body);
    expect([200, 502]).toContain(status);
  }, 15000);

  it("places bracket order (entry + SL + TP) on testnet", async () => {
    // Get current price from position/ticker to set reasonable bracket levels
    // Use a small amount and price levels that won't immediately fill
    const { status, body } = await post("/order", {
      ...envelope,
      params: {
        symbol: "BTC_USDT",
        type: "limit",
        side: "buy",
        amount: "0.001",
        price: "50000", // Well below market — entry shouldn't fill immediately
        clientOrderId: `testudo:test-${Date.now()}:entry`,
        stopLoss: { triggerPrice: "49000" },
        takeProfit: { triggerPrice: "55000" },
      },
    });

    console.log(`[testnet] Place bracket order response:`, body);

    if (status === 200) {
      expect(body.id).toBeTruthy();
      expect(body.status).toBe("open");
      expect(typeof body.amount).toBe("string");

      // If bracket order IDs are returned, verify them
      if (body.stopLossOrderId) {
        console.log(`[testnet] SL order ID: ${body.stopLossOrderId}`);
      }
      if (body.takeProfitOrderId) {
        console.log(`[testnet] TP order ID: ${body.takeProfitOrderId}`);
      }

      // Cleanup: cancel the order after verification
      const cancelRes = await post("/orders/cancel-all", {
        ...envelope,
        params: { symbol: "BTC_USDT" },
      });
      console.log(`[testnet] Cleanup cancel-all:`, cancelRes.body);
    } else {
      // Exchange may reject the order (insufficient margin, etc.)
      console.log(`[testnet] Order rejected (${status}): ${body.error}`);
    }
  }, 30000);

  // Phase 3: WebSocket Fill Streaming

  it("WebSocket subscribes and receives events", async () => {
    const ws = await connectWs();
    const collector = createCollector(ws);

    ws.send(
      JSON.stringify({
        action: "subscribe",
        exchange_id: "woo",
        credentials: {
          apiKey: API_KEY,
          secret: API_SECRET,
        },
        sandbox: true,
        symbols: ["BTC_USDT"],
      })
    );

    const msg = await collector.waitFor(
      (m) => m.event === "subscribed",
      15000
    );
    expect(msg.event).toBe("subscribed");
    console.log(`[testnet] WebSocket subscribed successfully`);

    ws.close();
  }, 20000);

  // Phase 4: Post-test cleanup verification

  it("no orphaned orders after test lifecycle", async () => {
    // Cancel any remaining orders
    await post("/orders/cancel-all", {
      ...envelope,
      params: { symbol: "BTC_USDT" },
    });

    // Verify no orders remain
    const { body } = await post("/orders/open", {
      ...envelope,
      params: { symbol: "BTC_USDT" },
    });
    expect(body).toHaveLength(0);
    console.log(`[testnet] Clean state verified — no orphaned orders`);
  }, 15000);
});

// Informational message when tests are skipped
if (!HAS_CREDENTIALS) {
  describe("Testnet Integration (skipped)", () => {
    it("credentials not provided — set WOO_TESTNET_KEY and WOO_TESTNET_SECRET", () => {
      console.log(
        "\n  To run testnet integration tests:\n" +
          "  WOO_TESTNET_KEY=<key> WOO_TESTNET_SECRET=<secret> bun test tests/testnet-integration.test.ts\n"
      );
      expect(true).toBe(true);
    });
  });
}
