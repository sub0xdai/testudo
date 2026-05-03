import { describe, it, expect, mock } from "bun:test";
import { createHandlers } from "../src/handlers";
import type { ExchangeGateway } from "../src/gateway";

function mockGateway(exchange: any): ExchangeGateway {
  return {
    getOrCreate: mock(() => Promise.resolve(exchange)),
    dispose: mock(() => Promise.resolve()),
    disposeAll: mock(() => Promise.resolve()),
    cacheKey: mock(() => "key"),
    size: 0,
    instances: new Map(),
  } as any;
}

function mockReq(body: any = {}): any { return { body }; }
function mockRes(): any {
  const res: any = { _status: 200, _json: null };
  res.status = (code: number) => { res._status = code; return res; };
  res.json = (data: any) => { res._json = data; return res; };
  return res;
}

function makeFill(execId: string, execTime: number): any {
  return {
    execId,
    symbol: "BTCUSDT",
    side: "Buy",
    execPrice: "50000",
    execQty: "0.01",
    execFee: "0.5",
    feeCurrency: "USDT",
    execTime: String(execTime),
    orderId: "order-" + execId,
  };
}

function makeBybitExchange(pages: any[][]): any {
  let callCount = 0;
  const xhrGet = mock((_path: string, opts: any) => {
    const pageIndex = callCount++;
    const list = pages[pageIndex] ?? [];
    const nextPageCursor =
      pageIndex < pages.length - 1 ? `cursor-${pageIndex + 1}` : undefined;
    return Promise.resolve({
      data: { result: { list, nextPageCursor } },
    });
  });
  return {
    store: { balance: {}, markets: [], orders: [], positions: [], loaded: {} },
    xhr: { get: xhrGet },
    placeOrder: mock(() => Promise.resolve([])),
    on: mock(() => {}),
    start: mock(() => Promise.resolve()),
    dispose: mock(() => {}),
  };
}

// Use a short window to stay within a single 7-day Bybit batch.
const SINCE_MS = 1746000000000;
const UNTIL_MS = SINCE_MS + 3_600_000; // 1-hour window → one Bybit batch

const baseEnvelope = {
  exchange_id: "bybit",
  credentials: { apiKey: "test-key", secret: "test-secret" },
  sandbox: false,
  params: {
    since_ms: SINCE_MS,
    until_ms: UNTIL_MS,
  },
};

describe("handleTradesSince — Bybit", () => {
  it("aggregates fills across three pages", async () => {
    const page1 = Array.from({ length: 100 }, (_, i) => makeFill(`e${i}`, 1746000000000 + i));
    const page2 = Array.from({ length: 100 }, (_, i) => makeFill(`e${100 + i}`, 1746000100000 + i));
    const page3 = [makeFill("e200", 1746000200000)]; // partial page → last page

    const exchange = makeBybitExchange([page1, page2, page3]);
    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesSince(mockReq(baseEnvelope), res);

    expect(res._status).toBe(200);
    expect(Array.isArray(res._json)).toBe(true);
    expect(res._json.length).toBe(201);
    expect(res._json[0].exec_id).toBe("e0");
    expect(res._json[200].exec_id).toBe("e200");
    // Verify wire format: all price fields are strings
    expect(typeof res._json[0].price).toBe("string");
    expect(typeof res._json[0].qty).toBe("string");
    expect(typeof res._json[0].fee).toBe("string");
    expect(res._json[0].side).toBe("buy");
    expect(res._json[0].fee_asset).toBe("USDT");
  });

  it("terminates early on empty first page", async () => {
    const exchange = makeBybitExchange([[]]); // one empty page
    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesSince(mockReq(baseEnvelope), res);

    expect(res._status).toBe(200);
    expect(res._json).toEqual([]);
    // xhr.get called exactly once — no further pages requested
    expect((exchange.xhr.get as ReturnType<typeof mock>).mock.calls.length).toBe(1);
  });

  it("guards against stuck cursor (same cursor repeated)", async () => {
    // page 1 full (100 items) with nextPageCursor "stuck"
    const page1 = Array.from({ length: 100 }, (_, i) => makeFill(`e${i}`, 1746000000000 + i));
    let callCount = 0;
    const xhrGet = mock((_path: string, _opts: any) => {
      callCount++;
      // Always return same cursor — simulates a stuck pagination bug
      return Promise.resolve({
        data: { result: { list: page1, nextPageCursor: "stuck-cursor" } },
      });
    });
    const exchange = {
      store: { balance: {}, markets: [], orders: [], positions: [], loaded: {} },
      xhr: { get: xhrGet },
      on: mock(() => {}),
      start: mock(() => Promise.resolve()),
      dispose: mock(() => {}),
    };
    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesSince(mockReq(baseEnvelope), res);

    expect(res._status).toBe(200);
    // Two calls: first (no cursor) → nextCursor="stuck-cursor"; second (cursor="stuck-cursor")
    // → nextCursor same as cursor → guard fires → break. One window, 2 xhr calls.
    expect(callCount).toBe(2);
    expect(res._json.length).toBe(200); // 100 fills × 2 calls
  });

  it("returns 501 for unsupported exchange", async () => {
    const exchange = makeBybitExchange([]);
    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesSince(
      mockReq({ ...baseEnvelope, exchange_id: "phemex" }),
      res
    );

    expect(res._status).toBe(501);
    expect(res._json.code).toBe("NotImplemented");
  });
});

describe("handleTradesSince — WOO", () => {
  it("aggregates WOO fills across two pages", async () => {
    function makeWooRow(id: number): any {
      return {
        id,
        symbol: "PERP_BTC_USDT",
        side: "BUY",
        executed_price: 50000,
        executed_quantity: 0.01,
        fee: 0.5,
        executed_timestamp: "1746000000.000",
        order_id: 100 + id,
      };
    }

    const page1 = Array.from({ length: 500 }, (_, i) => makeWooRow(i));
    const page2 = [makeWooRow(500)]; // partial → last page

    let pageCall = 0;
    const xhrGet = mock((_path: string, _opts: any) => {
      const rows = pageCall++ === 0 ? page1 : page2;
      return Promise.resolve({ data: { data: { rows } } });
    });

    const exchange = {
      store: { balance: {}, markets: [], orders: [], positions: [], loaded: {} },
      xhr: { get: xhrGet },
      on: mock(() => {}),
      start: mock(() => Promise.resolve()),
      dispose: mock(() => {}),
    };

    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesSince(
      mockReq({ ...baseEnvelope, exchange_id: "woo" }),
      res
    );

    expect(res._status).toBe(200);
    expect(res._json.length).toBe(501);
    expect(res._json[0].symbol).toBe("BTC_USDT");
    expect(res._json[0].fee_asset).toBe("USDT");
  });
});
