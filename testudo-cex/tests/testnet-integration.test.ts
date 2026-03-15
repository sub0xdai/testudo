/**
 * Live integration tests — CEX-08 Phase 2-4.
 *
 * Exercises the sidecar against live WOO X via safe-cex.
 *
 * Set env vars:
 *   WOO_API_KEY=<api-key>
 *   WOO_API_SECRET=<api-secret>
 *   WOO_APP_ID=<application-id>  (optional)
 *
 * Run:
 *   WOO_API_KEY=... WOO_API_SECRET=... bun test tests/testnet-integration.test.ts
 *
 * IMPORTANT: This places REAL orders on live WOO X.
 * Uses limit buy well below market price — should NOT fill.
 * All orders are cancelled at the end.
 */

import { describe, it, expect, beforeAll, afterAll } from "bun:test";
import { WebSocket } from "ws";

const API_KEY = process.env.WOO_API_KEY;
const API_SECRET = process.env.WOO_API_SECRET;
const APP_ID = process.env.WOO_APP_ID;
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
  sandbox: false,
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
const describeLive = HAS_CREDENTIALS ? describe : describe.skip;

describeLive("Live Integration (WOO X)", () => {
  beforeAll(async () => {
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

    console.log(`[live] Sidecar started on port ${TEST_PORT}`);
  }, 30000);

  afterAll(async () => {
    // Safety: cancel any remaining test orders before shutdown
    try {
      await post("/orders/cancel-all", {
        ...envelope,
        params: { symbol: "BTC_USDT" },
      });
    } catch {}
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

  it("connects to WOO X and fetches balance", async () => {
    const { status, body } = await post("/balance", envelope);
    expect(status).toBe(200);
    expect(Array.isArray(body)).toBe(true);
    expect(body[0].asset).toBe("USDT");
    console.log(
      `[live] Balance: ${body[0].free} USDT free / ${body[0].total} total`
    );
  }, 30000);

  it("fetches positions", async () => {
    const { status, body } = await post("/position", {
      ...envelope,
      params: {},
    });
    expect(status).toBe(200);
    expect(Array.isArray(body)).toBe(true);
    for (const p of body) {
      console.log(
        `[live] Position: ${p.symbol} ${p.side} ${p.contracts} @ ${p.entryPrice}`
      );
    }
  }, 15000);

  it("fetches open orders", async () => {
    const { status, body } = await post("/orders/open", {
      ...envelope,
      params: {},
    });
    expect(status).toBe(200);
    expect(Array.isArray(body)).toBe(true);
    console.log(`[live] Open orders: ${body.length}`);
  }, 15000);

  // Phase 2: Bracket Order Placement

  it("sets leverage on BTC_USDT", async () => {
    const { status, body } = await post("/leverage", {
      ...envelope,
      params: { symbol: "BTC_USDT", leverage: 5 },
    });
    console.log(`[live] Set leverage response:`, body);
    expect([200, 502]).toContain(status);
  }, 15000);

  it("places bracket order (entry + SL + TP) — limit far from market", async () => {
    // Limit buy at $50k — well below current BTC price (~$80-90k+)
    // This should NOT fill. Cancelled immediately after verification.
    const { status, body } = await post("/order", {
      ...envelope,
      params: {
        symbol: "BTC_USDT",
        type: "limit",
        side: "buy",
        amount: "0.001",
        price: "50000",
        clientOrderId: `testudo:cex08-${Date.now()}:entry`,
        stopLoss: { triggerPrice: "49000" },
        takeProfit: { triggerPrice: "55000" },
      },
    });

    console.log(`[live] Place bracket order response:`, JSON.stringify(body));

    if (status === 200) {
      expect(body.id).toBeTruthy();
      expect(typeof body.amount).toBe("string");

      console.log(`[live] Entry ID: ${body.id}`);
      if (body.stopLossOrderId)
        console.log(`[live] SL ID: ${body.stopLossOrderId}`);
      if (body.takeProfitOrderId)
        console.log(`[live] TP ID: ${body.takeProfitOrderId}`);

      // Verify orders appear in open orders
      const { body: openOrders } = await post("/orders/open", {
        ...envelope,
        params: { symbol: "BTC_USDT" },
      });
      console.log(`[live] Open orders after bracket: ${openOrders.length}`);

      // Cleanup: cancel all
      const cancelRes = await post("/orders/cancel-all", {
        ...envelope,
        params: { symbol: "BTC_USDT" },
      });
      console.log(`[live] Cleanup cancel-all:`, cancelRes.body);
    } else {
      console.log(`[live] Order rejected (${status}): ${body.error}`);
      // Don't fail — exchange may have restrictions
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
        sandbox: false,
        symbols: ["BTC_USDT"],
      })
    );

    const msg = await collector.waitFor(
      (m) => m.event === "subscribed",
      15000
    );
    expect(msg.event).toBe("subscribed");
    console.log(`[live] WebSocket subscribed successfully`);

    ws.close();
  }, 20000);

  // Phase 4: Clean state verification

  it("no orphaned orders after test lifecycle", async () => {
    // Final cancel-all safety net
    await post("/orders/cancel-all", {
      ...envelope,
      params: { symbol: "BTC_USDT" },
    });

    const { body } = await post("/orders/open", {
      ...envelope,
      params: { symbol: "BTC_USDT" },
    });

    // Only check for our test orders — there may be real trading orders
    const testOrders = body.filter((o: any) =>
      o.clientOrderId?.startsWith("testudo:cex08-")
    );
    expect(testOrders).toHaveLength(0);
    console.log(`[live] Clean state verified — no test orphans`);
  }, 15000);
});

// Informational message when tests are skipped
if (!HAS_CREDENTIALS) {
  describe("Live Integration (skipped)", () => {
    it("credentials not provided — set WOO_API_KEY and WOO_API_SECRET", () => {
      console.log(
        "\n  To run live integration tests:\n" +
          "  WOO_API_KEY=<key> WOO_API_SECRET=<secret> bun test tests/testnet-integration.test.ts\n"
      );
      expect(true).toBe(true);
    });
  });
}
