'use strict';

const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const ccxt = require('ccxt');
const { stringify } = require('./handlers');

// We test handlers by creating mock request/response objects and calling
// the handler functions directly, with a mocked pool.

/**
 * Create a mock response object that captures status and json.
 */
function mockRes() {
  const res = {
    _status: 200,
    _body: null,
    status(code) {
      res._status = code;
      return res;
    },
    json(body) {
      res._body = body;
      return res;
    },
  };
  return res;
}

/**
 * Create a mock exchange instance.
 */
function createMockExchange(overrides = {}) {
  return {
    fetchBalance: mock.fn(async () => ({
      info: {},
      total: { USDT: 1000.12345, BTC: 0.5 },
      free: { USDT: 800.5, BTC: 0.3 },
      used: { USDT: 199.62345, BTC: 0.2 },
    })),
    createOrder: mock.fn(async () => ({
      id: '12345',
      status: 'open',
      symbol: 'BTC/USDT:USDT',
      side: 'buy',
      type: 'limit',
      amount: 0.01,
      filled: 0,
      remaining: 0.01,
      average: null,
      price: 50000.5,
    })),
    editOrder: mock.fn(async () => ({
      id: '12345',
      status: 'open',
      symbol: 'BTC/USDT:USDT',
      side: 'buy',
      type: 'limit',
      amount: 0.02,
      filled: 0,
      remaining: 0.02,
      average: null,
      price: 51000.25,
    })),
    cancelOrder: mock.fn(async () => ({ id: '12345' })),
    fetchPositions: mock.fn(async () => [
      {
        symbol: 'BTC/USDT:USDT',
        side: 'long',
        contracts: 0.5,
        entryPrice: 49000.123,
        unrealizedPnl: 100.456,
      },
    ]),
    setLeverage: mock.fn(async () => ({})),
    ...overrides,
  };
}

// To test handlers, we need to intercept pool.getOrCreate.
// We do this by replacing the pool module's internals via a require hook approach.
// Instead, we'll test the handlers by directly requiring them and mocking the pool.

// Since Node.js test runner doesn't have module mocking built-in,
// we test at the integration level with a real pool entry injected.
const pool = require('./pool');
const handlers = require('./handlers');

const envelope = {
  exchange_id: 'binance',
  credentials: { apiKey: 'testkey', secret: 'testsecret' },
  sandbox: false,
  params: {},
};

describe('stringify', () => {
  it('converts numbers to strings', () => {
    assert.equal(stringify(42), '42');
    assert.equal(stringify(0.001), '0.001');
    assert.equal(stringify(1000.12345), '1000.12345');
  });

  it('preserves string input', () => {
    assert.equal(stringify('hello'), 'hello');
  });

  it('returns null for null', () => {
    assert.equal(stringify(null), null);
  });

  it('returns null for undefined', () => {
    assert.equal(stringify(undefined), null);
  });
});

describe('handleBalance', () => {
  beforeEach(() => pool.clear());
  afterEach(() => pool.clear());

  it('returns string decimals for balance values', async () => {
    // Pre-seed pool so getOrCreate returns our mock
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;

    // Temporarily replace getOrCreate
    pool.getOrCreate = () => mockExchange;

    const req = { body: { ...envelope, params: { type: 'future' } } };
    const res = mockRes();

    await handlers.handleBalance(req, res);

    assert.equal(res._status, 200);
    assert.ok(Array.isArray(res._body));
    assert.ok(res._body.length > 0);

    const usdt = res._body.find((b) => b.asset === 'USDT');
    assert.ok(usdt);
    assert.equal(typeof usdt.total, 'string');
    assert.equal(typeof usdt.free, 'string');
    assert.equal(typeof usdt.used, 'string');
    assert.equal(usdt.total, '1000.12345');
    assert.equal(usdt.free, '800.5');

    pool.getOrCreate = origGetOrCreate;
  });
});

describe('handleOrder', () => {
  beforeEach(() => pool.clear());
  afterEach(() => pool.clear());

  it('calls setLeverage when leverage > 0', async () => {
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: {
          symbol: 'BTC/USDT:USDT',
          type: 'limit',
          side: 'buy',
          amount: 0.01,
          price: 50000,
          leverage: 10,
        },
      },
    };
    const res = mockRes();

    await handlers.handleOrder(req, res);

    assert.equal(res._status, 200);
    assert.equal(mockExchange.setLeverage.mock.calls.length, 1);
    assert.deepEqual(mockExchange.setLeverage.mock.calls[0].arguments, [10, 'BTC/USDT:USDT']);
    assert.equal(mockExchange.createOrder.mock.calls.length, 1);

    // Verify response has string decimals
    assert.equal(typeof res._body.amount, 'string');
    assert.equal(typeof res._body.price, 'string');
    assert.equal(res._body.id, '12345');

    pool.getOrCreate = origGetOrCreate;
  });

  it('skips setLeverage when leverage not provided', async () => {
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: {
          symbol: 'BTC/USDT:USDT',
          type: 'market',
          side: 'buy',
          amount: 0.01,
        },
      },
    };
    const res = mockRes();

    await handlers.handleOrder(req, res);

    assert.equal(res._status, 200);
    assert.equal(mockExchange.setLeverage.mock.calls.length, 0);
    assert.equal(mockExchange.createOrder.mock.calls.length, 1);

    pool.getOrCreate = origGetOrCreate;
  });

  it('skips setLeverage when leverage is 0', async () => {
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: {
          symbol: 'BTC/USDT:USDT',
          type: 'market',
          side: 'buy',
          amount: 0.01,
          leverage: 0,
        },
      },
    };
    const res = mockRes();

    await handlers.handleOrder(req, res);

    assert.equal(mockExchange.setLeverage.mock.calls.length, 0);

    pool.getOrCreate = origGetOrCreate;
  });
});

describe('handleCancelOrder', () => {
  beforeEach(() => pool.clear());
  afterEach(() => pool.clear());

  it('returns success true', async () => {
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: { orderId: '12345', symbol: 'BTC/USDT:USDT' },
      },
    };
    const res = mockRes();

    await handlers.handleCancelOrder(req, res);

    assert.equal(res._status, 200);
    assert.deepEqual(res._body, { success: true });
    assert.equal(mockExchange.cancelOrder.mock.calls.length, 1);

    pool.getOrCreate = origGetOrCreate;
  });
});

describe('handlePosition', () => {
  beforeEach(() => pool.clear());
  afterEach(() => pool.clear());

  it('returns array with string decimals', async () => {
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: { symbol: 'BTC/USDT:USDT' },
      },
    };
    const res = mockRes();

    await handlers.handlePosition(req, res);

    assert.equal(res._status, 200);
    assert.ok(Array.isArray(res._body));
    assert.equal(res._body.length, 1);

    const pos = res._body[0];
    assert.equal(pos.symbol, 'BTC/USDT:USDT');
    assert.equal(pos.side, 'long');
    assert.equal(typeof pos.contracts, 'string');
    assert.equal(typeof pos.entryPrice, 'string');
    assert.equal(typeof pos.unrealizedPnl, 'string');
    assert.equal(pos.contracts, '0.5');
    assert.equal(pos.entryPrice, '49000.123');

    pool.getOrCreate = origGetOrCreate;
  });

  it('calls fetchPositions without symbol array when no symbol given', async () => {
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: {},
      },
    };
    const res = mockRes();

    await handlers.handlePosition(req, res);

    assert.equal(res._status, 200);
    // fetchPositions called without symbol filter
    const call = mockExchange.fetchPositions.mock.calls[0];
    assert.equal(call.arguments.length, 0);

    pool.getOrCreate = origGetOrCreate;
  });
});

describe('error mapping in handlers', () => {
  beforeEach(() => pool.clear());
  afterEach(() => pool.clear());

  it('maps CCXT AuthenticationError to 401 response', async () => {
    const mockExchange = createMockExchange({
      fetchBalance: mock.fn(async () => {
        throw new ccxt.AuthenticationError('invalid api key');
      }),
    });
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = { body: { ...envelope, params: {} } };
    const res = mockRes();

    await handlers.handleBalance(req, res);

    assert.equal(res._status, 401);
    assert.equal(res._body.code, 'AuthenticationError');
    assert.ok(res._body.error.includes('invalid api key'));

    pool.getOrCreate = origGetOrCreate;
  });

  it('maps CCXT InsufficientFunds to 402 response', async () => {
    const mockExchange = createMockExchange({
      createOrder: mock.fn(async () => {
        throw new ccxt.InsufficientFunds('not enough');
      }),
    });
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: { symbol: 'BTC/USDT:USDT', type: 'market', side: 'buy', amount: 100 },
      },
    };
    const res = mockRes();

    await handlers.handleOrder(req, res);

    assert.equal(res._status, 402);
    assert.equal(res._body.code, 'InsufficientFunds');

    pool.getOrCreate = origGetOrCreate;
  });
});

describe('handleEditOrder', () => {
  beforeEach(() => pool.clear());
  afterEach(() => pool.clear());

  it('returns order with string decimals', async () => {
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: {
          orderId: '12345',
          symbol: 'BTC/USDT:USDT',
          type: 'limit',
          side: 'buy',
          amount: 0.02,
          price: 51000.25,
        },
      },
    };
    const res = mockRes();

    await handlers.handleEditOrder(req, res);

    assert.equal(res._status, 200);
    assert.equal(res._body.id, '12345');
    assert.equal(typeof res._body.amount, 'string');
    assert.equal(typeof res._body.price, 'string');
    assert.equal(res._body.amount, '0.02');

    pool.getOrCreate = origGetOrCreate;
  });
});

describe('handleLeverage', () => {
  beforeEach(() => pool.clear());
  afterEach(() => pool.clear());

  it('returns success true', async () => {
    const mockExchange = createMockExchange();
    const origGetOrCreate = pool.getOrCreate;
    pool.getOrCreate = () => mockExchange;

    const req = {
      body: {
        ...envelope,
        params: { leverage: 20, symbol: 'BTC/USDT:USDT' },
      },
    };
    const res = mockRes();

    await handlers.handleLeverage(req, res);

    assert.equal(res._status, 200);
    assert.deepEqual(res._body, { success: true });
    assert.equal(mockExchange.setLeverage.mock.calls.length, 1);
    assert.deepEqual(mockExchange.setLeverage.mock.calls[0].arguments, [20, 'BTC/USDT:USDT']);

    pool.getOrCreate = origGetOrCreate;
  });
});
