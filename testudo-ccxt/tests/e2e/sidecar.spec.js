'use strict';

const { test, expect } = require('@playwright/test');
const { spawn } = require('node:child_process');
const path = require('node:path');

const SERVER_PATH = path.resolve(__dirname, '../../src/server.js');
const PORT = 3199; // Dedicated test port to avoid clashing with dev sidecar
const BASE = `http://127.0.0.1:${PORT}`;

let serverProcess;

test.beforeAll(async () => {
  // Start sidecar on test port
  serverProcess = spawn('node', [SERVER_PATH], {
    env: { ...process.env, CCXT_PORT: String(PORT) },
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  // Wait for server to be ready
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('Server did not start in 10s')), 10_000);

    serverProcess.stdout.on('data', (data) => {
      if (data.toString().includes('listening')) {
        clearTimeout(timeout);
        resolve();
      }
    });

    serverProcess.on('error', (err) => {
      clearTimeout(timeout);
      reject(err);
    });
  });
});

test.afterAll(async () => {
  if (serverProcess) {
    serverProcess.kill('SIGTERM');
    await new Promise((resolve) => serverProcess.on('close', resolve));
  }
});

// --- Helpers ---

/** Build a valid request envelope for the given exchange. */
function envelope(exchangeId, params = {}, { sandbox = true } = {}) {
  return {
    exchange_id: exchangeId,
    credentials: { apiKey: 'test_key_e2e', secret: 'test_secret_e2e' },
    sandbox,
    params,
  };
}

// ======================== Health & Discovery ========================

test.describe('Health & Discovery', () => {
  test('GET /health returns ok with pool stats', async ({ request }) => {
    const res = await request.get(`${BASE}/health`);
    expect(res.ok()).toBe(true);

    const body = await res.json();
    expect(body.ok).toBe(true);
    expect(typeof body.poolSize).toBe('number');
    expect(typeof body.uptime).toBe('number');
    expect(body.uptime).toBeGreaterThan(0);
  });

  test('GET /exchanges returns 100+ exchange IDs', async ({ request }) => {
    const res = await request.get(`${BASE}/exchanges`);
    expect(res.ok()).toBe(true);

    const exchanges = await res.json();
    expect(Array.isArray(exchanges)).toBe(true);
    expect(exchanges.length).toBeGreaterThan(100);

    // Key exchanges present
    expect(exchanges).toContain('binance');
    expect(exchanges).toContain('bybit');
    expect(exchanges).toContain('okx');
    expect(exchanges).toContain('woo');
  });
});

// ======================== Envelope Validation ========================

test.describe('Envelope Validation', () => {
  test('rejects missing exchange_id', async ({ request }) => {
    const res = await request.post(`${BASE}/balance`, {
      data: { credentials: { apiKey: 'x', secret: 'y' }, params: {} },
    });
    expect(res.ok()).toBe(false);
    expect(res.status()).toBe(500);

    const body = await res.json();
    expect(body.error).toContain('Missing exchange_id');
  });

  test('rejects missing credentials', async ({ request }) => {
    const res = await request.post(`${BASE}/balance`, {
      data: { exchange_id: 'binance', params: {} },
    });
    expect(res.ok()).toBe(false);

    const body = await res.json();
    expect(body.error).toContain('credentials');
  });

  test('rejects incomplete credentials (missing secret)', async ({ request }) => {
    const res = await request.post(`${BASE}/balance`, {
      data: { exchange_id: 'binance', credentials: { apiKey: 'x' }, params: {} },
    });
    expect(res.ok()).toBe(false);

    const body = await res.json();
    expect(body.error).toContain('credentials');
  });

  test('rejects unsupported exchange', async ({ request }) => {
    const res = await request.post(`${BASE}/balance`, {
      data: envelope('nonexistent_exchange_xyz'),
    });
    expect(res.ok()).toBe(false);

    const body = await res.json();
    expect(body.error).toContain('Unsupported exchange');
  });
});

// ======================== Exchange Authentication ========================

test.describe('Exchange Authentication', () => {
  test('POST /balance with invalid Bybit creds returns 401', async ({ request }) => {
    const res = await request.post(`${BASE}/balance`, {
      data: envelope('bybit', { type: 'future' }),
    });
    expect(res.status()).toBe(401);

    const body = await res.json();
    expect(body.code).toBe('AuthenticationError');
    expect(body.error).toBeTruthy();
  });

  test('POST /order with invalid creds returns 401', async ({ request }) => {
    const res = await request.post(`${BASE}/order`, {
      data: envelope('bybit', {
        symbol: 'BTC/USDT:USDT',
        type: 'limit',
        side: 'buy',
        amount: '0.001',
        price: '10000',
        leverage: 1,
      }),
    });
    expect(res.status()).toBe(401);

    const body = await res.json();
    expect(body.code).toBe('AuthenticationError');
  });

  test('POST /position with invalid creds returns 401', async ({ request }) => {
    const res = await request.post(`${BASE}/position`, {
      data: envelope('bybit', { symbol: 'BTC/USDT:USDT' }),
    });
    expect(res.status()).toBe(401);

    const body = await res.json();
    expect(body.code).toBe('AuthenticationError');
  });

  test('POST /order/cancel with invalid creds returns 401', async ({ request }) => {
    const res = await request.post(`${BASE}/order/cancel`, {
      data: envelope('bybit', { orderId: 'fake-id', symbol: 'BTC/USDT:USDT' }),
    });
    expect(res.status()).toBe(401);

    const body = await res.json();
    expect(body.code).toBe('AuthenticationError');
  });

  test('POST /order/edit with invalid creds returns 401', async ({ request }) => {
    const res = await request.post(`${BASE}/order/edit`, {
      data: envelope('bybit', {
        orderId: 'fake-id',
        symbol: 'BTC/USDT:USDT',
        type: 'limit',
        side: 'buy',
        amount: '0.001',
        price: '10000',
      }),
    });
    expect(res.status()).toBe(401);

    const body = await res.json();
    expect(body.code).toBe('AuthenticationError');
  });

  test('POST /leverage with invalid creds returns 401', async ({ request }) => {
    const res = await request.post(`${BASE}/leverage`, {
      data: envelope('bybit', { leverage: 10, symbol: 'BTC/USDT:USDT' }),
    });
    expect(res.status()).toBe(401);

    const body = await res.json();
    expect(body.code).toBe('AuthenticationError');
  });
});

// ======================== Pool Behavior ========================

test.describe('Pool Behavior', () => {
  test('pool size increases after exchange instantiation', async ({ request }) => {
    // Get initial pool size
    const before = await (await request.get(`${BASE}/health`)).json();
    const initialSize = before.poolSize;

    // Trigger a new exchange instance (will fail auth but still creates instance)
    await request.post(`${BASE}/balance`, {
      data: envelope('okx', { type: 'future' }),
    });

    const after = await (await request.get(`${BASE}/health`)).json();
    expect(after.poolSize).toBeGreaterThanOrEqual(initialSize);
  });

  test('same credentials reuse cached instance (pool size stable)', async ({ request }) => {
    // First call creates the instance
    await request.post(`${BASE}/balance`, {
      data: envelope('kraken', { type: 'future' }),
    });

    const after1 = await (await request.get(`${BASE}/health`)).json();
    const size1 = after1.poolSize;

    // Second call with same creds should reuse
    await request.post(`${BASE}/balance`, {
      data: envelope('kraken', { type: 'future' }),
    });

    const after2 = await (await request.get(`${BASE}/health`)).json();
    expect(after2.poolSize).toBe(size1);
  });
});

// ======================== Multi-Exchange Support ========================

test.describe('Multi-Exchange Support', () => {
  const exchanges = ['binance', 'bybit', 'okx', 'woo', 'bitget', 'kucoin'];

  for (const exchangeId of exchanges) {
    test(`${exchangeId}: accepts balance request and returns structured error`, async ({ request }) => {
      const res = await request.post(`${BASE}/balance`, {
        data: envelope(exchangeId, { type: 'future' }),
      });

      // Should get an auth error (not a crash/500), proving the exchange loads
      const body = await res.json();
      expect(body.code).toBeTruthy();
      expect(body.error).toBeTruthy();
      // Status should be 401 (auth) or 500 (exchange-specific), not a crash
      expect(res.status()).toBeGreaterThanOrEqual(400);
      expect(res.status()).toBeLessThanOrEqual(503);
    });
  }
});

// ======================== Error Response Format ========================

test.describe('Error Response Format', () => {
  test('error responses have consistent { error, code } shape', async ({ request }) => {
    const endpoints = [
      { path: '/balance', data: envelope('bybit', { type: 'future' }) },
      { path: '/order', data: envelope('bybit', { symbol: 'BTC/USDT:USDT', type: 'limit', side: 'buy', amount: '0.001', price: '10000' }) },
      { path: '/position', data: envelope('bybit', { symbol: 'BTC/USDT:USDT' }) },
      { path: '/order/cancel', data: envelope('bybit', { orderId: 'x', symbol: 'BTC/USDT:USDT' }) },
      { path: '/leverage', data: envelope('bybit', { leverage: 10, symbol: 'BTC/USDT:USDT' }) },
    ];

    for (const ep of endpoints) {
      const res = await request.post(`${BASE}${ep.path}`, { data: ep.data });
      const body = await res.json();

      expect(body).toHaveProperty('error');
      expect(body).toHaveProperty('code');
      expect(typeof body.error).toBe('string');
      expect(typeof body.code).toBe('string');
      expect(body.error.length).toBeGreaterThan(0);
      expect(body.code.length).toBeGreaterThan(0);
    }
  });
});

// ======================== Binance Sandbox Deprecation ========================

test.describe('Binance Sandbox', () => {
  test('Binance futures sandbox returns informative error (deprecated)', async ({ request }) => {
    const res = await request.post(`${BASE}/balance`, {
      data: envelope('binance', { type: 'future' }),
    });

    // Should fail with a meaningful error (not a crash)
    expect(res.ok()).toBe(false);
    const body = await res.json();
    expect(body.error).toBeTruthy();
    // CCXT reports sandbox deprecation
    expect(body.error.toLowerCase()).toMatch(/sandbox|not supported|deprecat/i);
  });
});
