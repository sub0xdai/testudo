import { describe, it, expect, mock, beforeEach } from "bun:test";
import {
  processPending,
  sendOrderUpdate,
  snapshotOrder,
  type OrderSnapshot,
  type OrderUpdatePayload,
} from "../src/ws-fills";
import type { OrderFillEvent } from "safe-cex/dist/types";
import type { WebSocket } from "ws";

// ── Mock WebSocket ──

function mockWs(): WebSocket & { _messages: string[] } {
  const messages: string[] = [];
  return {
    readyState: 1, // OPEN
    OPEN: 1,
    send: mock((data: string) => messages.push(data)),
    _messages: messages,
  } as any;
}

// ── Helpers ──

function parseMessages(ws: ReturnType<typeof mockWs>): any[] {
  return ws._messages.map((m) => JSON.parse(m));
}

function makeOrder(overrides: Partial<OrderSnapshot> = {}): OrderSnapshot {
  return {
    id: "order-1",
    symbol: "BTC_USDT",
    side: "buy",
    price: 70000,
    amount: 0.01,
    filled: 0,
    remaining: 0.01,
    ...overrides,
  };
}

function makeFill(overrides: Partial<OrderFillEvent> = {}): OrderFillEvent {
  return {
    symbol: "BTC_USDT",
    side: "buy" as any,
    price: 70050,
    amount: 0.01,
    ...overrides,
  };
}

// ── Tests ──

describe("snapshotOrder", () => {
  it("extracts required fields from a safe-cex Order", () => {
    const order = {
      id: "abc",
      symbol: "ETH_USDT",
      side: "sell",
      price: 3500,
      amount: 0.5,
      filled: 0.1,
      remaining: 0.4,
      type: "limit",
      status: "open",
      reduceOnly: false,
    };
    const snap = snapshotOrder(order);
    expect(snap).toEqual({
      id: "abc",
      symbol: "ETH_USDT",
      side: "sell",
      price: 3500,
      amount: 0.5,
      filled: 0.1,
      remaining: 0.4,
    });
  });

  it("defaults filled to 0 and remaining to amount when missing", () => {
    const snap = snapshotOrder({ id: "x", symbol: "A", side: "buy", price: 1, amount: 5 });
    expect(snap.filled).toBe(0);
    expect(snap.remaining).toBe(5);
  });
});

describe("sendOrderUpdate", () => {
  it("sends JSON envelope with event: order_update", () => {
    const ws = mockWs();
    const data: OrderUpdatePayload = {
      id: "order-1",
      symbol: "BTC_USDT",
      status: "closed",
      side: "buy",
      price: 70000,
      amount: 0.01,
      filled: 0.01,
      remaining: 0,
      average: 70050,
      timestamp: 1710500000000,
    };
    sendOrderUpdate(ws, data);

    const msgs = parseMessages(ws);
    expect(msgs).toHaveLength(1);
    expect(msgs[0].event).toBe("order_update");
    expect(msgs[0].data).toEqual(data);
  });

  it("does not send when ws is not OPEN", () => {
    const ws = mockWs();
    (ws as any).readyState = 3; // CLOSED
    sendOrderUpdate(ws, {
      id: "x",
      symbol: "X",
      status: "closed",
      side: "buy",
      price: 0,
      amount: 0,
      filled: 0,
      remaining: 0,
      average: 0,
      timestamp: 0,
    });
    expect(ws._messages).toHaveLength(0);
  });

  it("event shape matches Rust OrderUpdateEvent struct", () => {
    const ws = mockWs();
    sendOrderUpdate(ws, {
      id: "exchange-order-123",
      symbol: "BTC/USDT:USDT",
      status: "closed",
      side: "buy",
      price: 70000.5,
      amount: 0.01,
      filled: 0.01,
      remaining: 0,
      average: 70050.25,
      timestamp: 1710500000000,
    });

    const msg = parseMessages(ws)[0];

    // Verify envelope structure
    expect(msg.event).toBe("order_update");
    expect(msg.data).toBeDefined();

    // Verify all fields exist with correct types (matches Rust struct)
    const d = msg.data;
    expect(typeof d.id).toBe("string");
    expect(typeof d.symbol).toBe("string");
    expect(typeof d.status).toBe("string");
    expect(typeof d.side).toBe("string");
    expect(typeof d.price).toBe("number"); // Rust: Option<f64>
    expect(typeof d.amount).toBe("number"); // Rust: Option<f64>
    expect(typeof d.filled).toBe("number"); // Rust: Option<f64>
    expect(typeof d.remaining).toBe("number"); // Rust: Option<f64>
    expect(typeof d.average).toBe("number"); // Rust: Option<f64>
    expect(typeof d.timestamp).toBe("number"); // Rust: Option<i64>
  });
});

describe("processPending", () => {
  describe("fill + removal = closed (regular order: update fires before fill)", () => {
    it("matches a fill to a pending removal and emits status: closed", () => {
      const ws = mockWs();
      const fills: OrderFillEvent[] = [makeFill()];
      const removals = new Map<string, OrderSnapshot>([
        ["order-1", makeOrder()],
      ]);

      processPending(ws, fills, removals);

      const msgs = parseMessages(ws);
      expect(msgs).toHaveLength(1);
      expect(msgs[0].data.id).toBe("order-1");
      expect(msgs[0].data.status).toBe("closed");
      expect(msgs[0].data.side).toBe("buy");
      expect(msgs[0].data.symbol).toBe("BTC_USDT");
      expect(msgs[0].data.average).toBe(70050); // fill price
      expect(msgs[0].data.remaining).toBe(0);
    });

    it("clears fills and removals after processing", () => {
      const ws = mockWs();
      const fills: OrderFillEvent[] = [makeFill()];
      const removals = new Map([["order-1", makeOrder()]]);

      processPending(ws, fills, removals);

      expect(fills).toHaveLength(0);
      expect(removals.size).toBe(0);
    });
  });

  describe("fill for algo order (fill fires before update)", () => {
    it("also produces closed event when matched in batch", () => {
      const ws = mockWs();
      // Algo order: fill arrives, then removal arrives — both in same batch
      const fills: OrderFillEvent[] = [
        makeFill({ symbol: "ETH_USDT", side: "sell" as any, price: 3500, amount: 0.5 }),
      ];
      const removals = new Map([
        [
          "algo-sl-1",
          makeOrder({ id: "algo-sl-1", symbol: "ETH_USDT", side: "sell", price: 3500, amount: 0.5 }),
        ],
      ]);

      processPending(ws, fills, removals);

      const msgs = parseMessages(ws);
      expect(msgs).toHaveLength(1);
      expect(msgs[0].data.id).toBe("algo-sl-1");
      expect(msgs[0].data.status).toBe("closed");
      expect(msgs[0].data.symbol).toBe("ETH_USDT");
    });
  });

  describe("removal without fill = canceled", () => {
    it("emits status: canceled for removed order with no matching fill", () => {
      const ws = mockWs();
      const fills: OrderFillEvent[] = [];
      const removals = new Map([
        ["tp-order-1", makeOrder({ id: "tp-order-1", symbol: "BTC_USDT", side: "sell" })],
      ]);

      processPending(ws, fills, removals);

      const msgs = parseMessages(ws);
      expect(msgs).toHaveLength(1);
      expect(msgs[0].data.id).toBe("tp-order-1");
      expect(msgs[0].data.status).toBe("canceled");
      expect(msgs[0].data.symbol).toBe("BTC_USDT");
    });

    it("preserves filled/remaining from order snapshot", () => {
      const ws = mockWs();
      const fills: OrderFillEvent[] = [];
      const removals = new Map([
        [
          "partial-cancel",
          makeOrder({ id: "partial-cancel", filled: 0.005, remaining: 0.005 }),
        ],
      ]);

      processPending(ws, fills, removals);

      const d = parseMessages(ws)[0].data;
      expect(d.filled).toBe(0.005);
      expect(d.remaining).toBe(0.005);
    });
  });

  describe("fill without removal = partial fill (ignored)", () => {
    it("does not emit any event for a fill with no matching removal", () => {
      const ws = mockWs();
      const fills: OrderFillEvent[] = [makeFill()];
      const removals = new Map<string, OrderSnapshot>(); // No removals

      processPending(ws, fills, removals);

      expect(ws._messages).toHaveLength(0);
    });
  });

  describe("multiple orders in batch", () => {
    it("handles mixed fills and cancellations in single batch", () => {
      const ws = mockWs();

      // Two removals: one matched by a fill, one cancelled
      const fills: OrderFillEvent[] = [
        makeFill({ symbol: "BTC_USDT", side: "buy" as any, price: 70050, amount: 0.01 }),
      ];
      const removals = new Map<string, OrderSnapshot>([
        ["entry-1", makeOrder({ id: "entry-1", symbol: "BTC_USDT", side: "buy" })],
        ["tp-1", makeOrder({ id: "tp-1", symbol: "ETH_USDT", side: "sell" })],
      ]);

      processPending(ws, fills, removals);

      const msgs = parseMessages(ws);
      expect(msgs).toHaveLength(2);

      // entry-1 matched to fill → closed
      const closed = msgs.find((m: any) => m.data.status === "closed");
      expect(closed.data.id).toBe("entry-1");

      // tp-1 unmatched → canceled
      const canceled = msgs.find((m: any) => m.data.status === "canceled");
      expect(canceled.data.id).toBe("tp-1");
    });

    it("matches fill to correct order by symbol + side", () => {
      const ws = mockWs();

      // Two orders on same symbol, different sides
      const fills: OrderFillEvent[] = [
        makeFill({ symbol: "BTC_USDT", side: "sell" as any }),
      ];
      const removals = new Map<string, OrderSnapshot>([
        ["buy-order", makeOrder({ id: "buy-order", symbol: "BTC_USDT", side: "buy" })],
        ["sell-order", makeOrder({ id: "sell-order", symbol: "BTC_USDT", side: "sell" })],
      ]);

      processPending(ws, fills, removals);

      const msgs = parseMessages(ws);
      const closed = msgs.find((m: any) => m.data.status === "closed");
      const canceled = msgs.find((m: any) => m.data.status === "canceled");

      expect(closed.data.id).toBe("sell-order"); // Matched by side
      expect(canceled.data.id).toBe("buy-order"); // Not matched
    });

    it("handles two fills with two removals correctly", () => {
      const ws = mockWs();

      const fills: OrderFillEvent[] = [
        makeFill({ symbol: "BTC_USDT", side: "buy" as any, price: 70050 }),
        makeFill({ symbol: "ETH_USDT", side: "sell" as any, price: 3510 }),
      ];
      const removals = new Map<string, OrderSnapshot>([
        ["btc-entry", makeOrder({ id: "btc-entry", symbol: "BTC_USDT", side: "buy" })],
        ["eth-tp", makeOrder({ id: "eth-tp", symbol: "ETH_USDT", side: "sell" })],
      ]);

      processPending(ws, fills, removals);

      const msgs = parseMessages(ws);
      expect(msgs).toHaveLength(2);
      expect(msgs.every((m: any) => m.data.status === "closed")).toBe(true);

      const ids = msgs.map((m: any) => m.data.id).sort();
      expect(ids).toEqual(["btc-entry", "eth-tp"]);
    });
  });
});

describe("setupFillStreaming integration", () => {
  // Integration-style tests using mocked gateway and WebSocket

  it("sends subscribed acknowledgment on valid subscribe message", async () => {
    const { setupFillStreaming } = await import("../src/ws-fills");

    const ws = mockWs();
    const connectionHandlers: Function[] = [];
    const wss = {
      on: mock((event: string, handler: Function) => {
        if (event === "connection") connectionHandlers.push(handler);
      }),
    } as any;

    const mockExchange = {
      on: mock(() => {}),
      off: mock(() => {}),
      store: { orders: [] },
    };

    const gateway = {
      getOrCreate: mock(() => Promise.resolve(mockExchange)),
    } as any;

    setupFillStreaming(wss, gateway);

    // Simulate connection
    const messageHandlers: Function[] = [];
    const closeHandlers: Function[] = [];
    (ws as any).on = mock((event: string, handler: Function) => {
      if (event === "message") messageHandlers.push(handler);
      if (event === "close") closeHandlers.push(handler);
    });

    connectionHandlers[0](ws);

    // Send subscribe message
    const subscribeMsg = JSON.stringify({
      action: "subscribe",
      exchange_id: "woo",
      credentials: { apiKey: "test-key", secret: "test-secret" },
      sandbox: false,
      symbols: ["BTC_USDT"],
    });

    await messageHandlers[0](subscribeMsg);

    const msgs = parseMessages(ws);
    const ackMsg = msgs.find((m) => m.event === "subscribed");
    expect(ackMsg).toBeDefined();
    expect(ackMsg.message).toBe("Streaming order updates");
  });

  it("wires fill and update listeners on the exchange", async () => {
    const { setupFillStreaming } = await import("../src/ws-fills");

    const ws = mockWs();
    const connectionHandlers: Function[] = [];
    const wss = {
      on: mock((event: string, handler: Function) => {
        if (event === "connection") connectionHandlers.push(handler);
      }),
    } as any;

    const mockExchange = {
      on: mock(() => {}),
      off: mock(() => {}),
      store: { orders: [] },
    };

    const gateway = {
      getOrCreate: mock(() => Promise.resolve(mockExchange)),
    } as any;

    setupFillStreaming(wss, gateway);

    const messageHandlers: Function[] = [];
    const closeHandlers: Function[] = [];
    (ws as any).on = mock((event: string, handler: Function) => {
      if (event === "message") messageHandlers.push(handler);
      if (event === "close") closeHandlers.push(handler);
    });

    connectionHandlers[0](ws);

    await messageHandlers[0](
      JSON.stringify({
        action: "subscribe",
        exchange_id: "woo",
        credentials: { apiKey: "k", secret: "s" },
        sandbox: false,
        symbols: [],
      })
    );

    const onCalls = mockExchange.on.mock.calls;
    const events = onCalls.map((c: any[]) => c[0]);
    expect(events).toContain("fill");
    expect(events).toContain("update");
  });

  it("cleans up listeners on WebSocket close", async () => {
    const { setupFillStreaming } = await import("../src/ws-fills");

    const ws = mockWs();
    const connectionHandlers: Function[] = [];
    const wss = {
      on: mock((event: string, handler: Function) => {
        if (event === "connection") connectionHandlers.push(handler);
      }),
    } as any;

    const mockExchange = {
      on: mock(() => {}),
      off: mock(() => {}),
      store: { orders: [] },
    };

    const gateway = {
      getOrCreate: mock(() => Promise.resolve(mockExchange)),
    } as any;

    setupFillStreaming(wss, gateway);

    const messageHandlers: Function[] = [];
    const closeHandlers: Function[] = [];
    (ws as any).on = mock((event: string, handler: Function) => {
      if (event === "message") messageHandlers.push(handler);
      if (event === "close") closeHandlers.push(handler);
    });

    connectionHandlers[0](ws);

    await messageHandlers[0](
      JSON.stringify({
        action: "subscribe",
        exchange_id: "woo",
        credentials: { apiKey: "k", secret: "s" },
        sandbox: false,
        symbols: [],
      })
    );

    // Simulate disconnect
    closeHandlers[0]();

    const offCalls = mockExchange.off.mock.calls;
    const offEvents = offCalls.map((c: any[]) => c[0]);
    expect(offEvents).toContain("fill");
    expect(offEvents).toContain("update");
  });

  it("ignores non-subscribe messages", async () => {
    const { setupFillStreaming } = await import("../src/ws-fills");

    const ws = mockWs();
    const connectionHandlers: Function[] = [];
    const wss = {
      on: mock((event: string, handler: Function) => {
        if (event === "connection") connectionHandlers.push(handler);
      }),
    } as any;

    const gateway = {
      getOrCreate: mock(() => Promise.reject(new Error("should not be called"))),
    } as any;

    setupFillStreaming(wss, gateway);

    const messageHandlers: Function[] = [];
    (ws as any).on = mock((event: string, handler: Function) => {
      if (event === "message") messageHandlers.push(handler);
    });

    connectionHandlers[0](ws);

    // Send a non-subscribe message — should be ignored
    await messageHandlers[0](JSON.stringify({ action: "ping" }));

    expect(ws._messages).toHaveLength(0);
  });
});
