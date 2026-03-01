'use strict';

const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { handleOrdersConnection } = require('./ws-orders');

/**
 * Create a mock WebSocket that captures sent messages.
 */
function mockWs() {
  const handlers = {};
  const ws = {
    readyState: 1, // OPEN
    _sent: [],
    send(data) {
      ws._sent.push(JSON.parse(data));
    },
    on(event, fn) {
      handlers[event] = fn;
    },
    _emit(event, data) {
      if (handlers[event]) handlers[event](data);
    },
  };
  return ws;
}

describe('handleOrdersConnection', () => {
  it('sends error on invalid JSON', () => {
    const ws = mockWs();
    handleOrdersConnection(ws);
    ws._emit('message', 'not json');
    assert.equal(ws._sent.length, 1);
    assert.equal(ws._sent[0].event, 'error');
    assert.ok(ws._sent[0].message.includes('Invalid JSON'));
  });

  it('sends error when credentials are missing', async () => {
    const ws = mockWs();
    handleOrdersConnection(ws);
    ws._emit('message', JSON.stringify({ action: 'subscribe', exchange_id: 'woo' }));
    // Allow async to settle
    await new Promise((r) => setTimeout(r, 10));
    const errors = ws._sent.filter((m) => m.event === 'error');
    assert.ok(errors.length > 0);
    assert.ok(errors[0].message.includes('credentials'));
  });

  it('sends error when symbols is empty', async () => {
    const ws = mockWs();
    handleOrdersConnection(ws);
    ws._emit('message', JSON.stringify({
      action: 'subscribe',
      exchange_id: 'woo',
      credentials: { apiKey: 'k', secret: 's' },
      symbols: [],
    }));
    await new Promise((r) => setTimeout(r, 10));
    const errors = ws._sent.filter((m) => m.event === 'error');
    assert.ok(errors.length > 0);
    assert.ok(errors[0].message.includes('symbols'));
  });

  it('sends subscribed event with valid params and mock exchange', async () => {
    const pool = require('./pool');
    const origGetOrCreate = pool.getOrCreate;

    const mockExchange = {
      id: 'woo',
      watchOrders: mock.fn(async () => {
        // Return one mock order, then hang forever to avoid tight loop
        await new Promise(() => {}); // never resolves
        return [];
      }),
    };
    pool.getOrCreate = () => mockExchange;

    const ws = mockWs();
    handleOrdersConnection(ws);
    ws._emit('message', JSON.stringify({
      action: 'subscribe',
      exchange_id: 'woo',
      credentials: { apiKey: 'k', secret: 's' },
      sandbox: false,
      symbols: ['BTC/USDT:USDT'],
    }));

    await new Promise((r) => setTimeout(r, 50));

    const subscribed = ws._sent.find((m) => m.event === 'subscribed');
    assert.ok(subscribed);
    assert.deepEqual(subscribed.symbols, ['BTC/USDT:USDT']);

    pool.getOrCreate = origGetOrCreate;
  });

  it('sends order_update events when watchOrders returns orders', async () => {
    const pool = require('./pool');
    const origGetOrCreate = pool.getOrCreate;
    let callCount = 0;

    const mockExchange = {
      id: 'woo',
      watchOrders: mock.fn(async () => {
        callCount++;
        if (callCount === 1) {
          return [{
            id: '12345',
            symbol: 'BTC/USDT:USDT',
            status: 'closed',
            side: 'buy',
            price: 50000,
            amount: 0.1,
            filled: 0.1,
            remaining: 0,
            average: 49998.5,
            timestamp: 1709280000000,
          }];
        }
        // Hang after first response to avoid tight loop
        await new Promise(() => {});
        return [];
      }),
    };
    pool.getOrCreate = () => mockExchange;

    const ws = mockWs();
    handleOrdersConnection(ws);
    ws._emit('message', JSON.stringify({
      action: 'subscribe',
      exchange_id: 'woo',
      credentials: { apiKey: 'k', secret: 's' },
      sandbox: false,
      symbols: ['BTC/USDT:USDT'],
    }));

    await new Promise((r) => setTimeout(r, 100));

    const updates = ws._sent.filter((m) => m.event === 'order_update');
    assert.ok(updates.length >= 1);
    assert.equal(updates[0].data.id, '12345');
    assert.equal(updates[0].data.status, 'closed');
    assert.equal(updates[0].data.symbol, 'BTC/USDT:USDT');
    assert.equal(updates[0].data.filled, 0.1);

    pool.getOrCreate = origGetOrCreate;
  });

  it('sends error when watchOrders is not supported', async () => {
    const pool = require('./pool');
    const origGetOrCreate = pool.getOrCreate;

    const mockExchange = { id: 'testex' }; // no watchOrders method
    pool.getOrCreate = () => mockExchange;

    const ws = mockWs();
    handleOrdersConnection(ws);
    ws._emit('message', JSON.stringify({
      action: 'subscribe',
      exchange_id: 'testex',
      credentials: { apiKey: 'k', secret: 's' },
      sandbox: false,
      symbols: ['BTC/USDT:USDT'],
    }));

    await new Promise((r) => setTimeout(r, 50));

    const errors = ws._sent.filter((m) => m.event === 'error');
    assert.ok(errors.some((e) => e.message.includes('watchOrders not supported')));

    pool.getOrCreate = origGetOrCreate;
  });

  it('stops watching on close', async () => {
    const pool = require('./pool');
    const origGetOrCreate = pool.getOrCreate;

    const mockExchange = {
      id: 'woo',
      watchOrders: mock.fn(async () => {
        await new Promise(() => {});
        return [];
      }),
    };
    pool.getOrCreate = () => mockExchange;

    const ws = mockWs();
    handleOrdersConnection(ws);
    ws._emit('message', JSON.stringify({
      action: 'subscribe',
      exchange_id: 'woo',
      credentials: { apiKey: 'k', secret: 's' },
      sandbox: false,
      symbols: ['BTC/USDT:USDT'],
    }));

    await new Promise((r) => setTimeout(r, 20));

    // Simulate close
    ws._emit('close');

    // Should not crash or keep running
    pool.getOrCreate = origGetOrCreate;
  });
});
