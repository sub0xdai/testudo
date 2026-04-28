import { describe, it, expect, mock } from "bun:test";
import { createHandlers } from "../src/handlers";
import type { ExchangeGateway } from "../src/gateway";

function mockXhrGet(responseData: any) {
  return mock(() => Promise.resolve({ data: responseData }));
}

function makeExchangeWithExecutions(executions: any[]) {
  return {
    store: { balance: {}, markets: [], orders: [], positions: [], loaded: {} },
    xhr: { get: mockXhrGet({ result: { list: executions } }) },
    placeOrder: mock(() => Promise.resolve([])),
    on: mock(() => {}),
    start: mock(() => Promise.resolve()),
    dispose: mock(() => {}),
  };
}

function mockGateway(exchange: any): ExchangeGateway {
  return {
    getOrCreate: mock(() => Promise.resolve(exchange)),
    dispose: mock(() => Promise.resolve()),
    disposeAll: mock(() => Promise.resolve()),
    cacheKey: mock(() => 'key'),
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

const bybitEnvelope = {
  exchange_id: 'bybit',
  credentials: { apiKey: 'test-key', secret: 'test-secret' },
  sandbox: false,
  params: {
    symbol: 'ETH_USDT',
    since_ms: 1745680000000,
    until_ms: 1745680200000,
    expected_qty: '0.09',
    qty_tolerance: '0.001',
    entry_side: 'sell',  // SHORT trade: close side is 'buy'
  },
};

describe("handleTradesByGroup", () => {
  it("returns matched trade for close side + qty", async () => {
    const executions = [
      // wrong side
      { orderId: 'sl-111', side: 'Sell', execPrice: '2419', execQty: '0.09', execTime: '1745680100000' },
      // correct side + qty
      { orderId: 'tp-222', side: 'Buy', execPrice: '2369.78', execQty: '0.09', execTime: '1745680112000' },
      // correct side but wrong qty
      { orderId: 'other-333', side: 'Buy', execPrice: '2370', execQty: '0.05', execTime: '1745680120000' },
    ];
    const exchange = makeExchangeWithExecutions(executions);
    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesByGroup(mockReq(bybitEnvelope), res);

    expect(res._status).toBe(200);
    expect(res._json.matched).not.toBeNull();
    expect(res._json.matched.order_id).toBe('tp-222');
    expect(res._json.matched.avg_price).toBe('2369.78');
    expect(res._json.matched.filled_qty).toBe('0.09');
    expect(res._json.matched.side).toBe('buy');
  });

  it("returns null when no trade matches", async () => {
    const executions = [
      { orderId: 'x-1', side: 'Sell', execPrice: '2419', execQty: '0.09', execTime: '1745680100000' },
    ];
    const exchange = makeExchangeWithExecutions(executions);
    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesByGroup(mockReq(bybitEnvelope), res);

    expect(res._status).toBe(200);
    expect(res._json.matched).toBeNull();
  });

  it("returns most recent when multiple matches", async () => {
    const executions = [
      { orderId: 'old-111', side: 'Buy', execPrice: '2369.00', execQty: '0.09', execTime: '1745680100000' },
      { orderId: 'new-222', side: 'Buy', execPrice: '2369.78', execQty: '0.09', execTime: '1745680112000' },
    ];
    const exchange = makeExchangeWithExecutions(executions);
    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesByGroup(mockReq(bybitEnvelope), res);

    expect(res._json.matched.order_id).toBe('new-222');
  });

  it("returns 501 for non-Bybit exchange", async () => {
    const exchange = makeExchangeWithExecutions([]);
    const handlers = createHandlers(mockGateway(exchange));
    const res = mockRes();

    await handlers.handleTradesByGroup(mockReq({
      ...bybitEnvelope,
      exchange_id: 'binance',
    }), res);

    expect(res._status).toBe(501);
  });
});
