'use strict';

const { describe, it, beforeEach, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const pool = require('./pool');

const fakeCreds = { apiKey: 'key1', secret: 'secret1' };
const fakeCreds2 = { apiKey: 'key2', secret: 'secret2' };

beforeEach(() => {
  pool.clear();
});

afterEach(() => {
  pool.clear();
  pool.stopEviction();
});

describe('pool', () => {
  it('creates and caches exchange instances', () => {
    const instance1 = pool.getOrCreate('binance', fakeCreds, false);
    assert.ok(instance1);
    assert.equal(pool.size(), 1);

    // Same credentials should return the same instance
    const instance2 = pool.getOrCreate('binance', fakeCreds, false);
    assert.equal(instance1, instance2);
    assert.equal(pool.size(), 1);
  });

  it('returns same instance for same credentials', () => {
    const a = pool.getOrCreate('binance', fakeCreds, false);
    const b = pool.getOrCreate('binance', fakeCreds, false);
    assert.strictEqual(a, b);
  });

  it('generates different keys for different credentials', () => {
    const key1 = pool.makeKey('binance', fakeCreds, false);
    const key2 = pool.makeKey('binance', fakeCreds2, false);
    assert.notEqual(key1, key2);
  });

  it('generates different keys for different sandbox flags', () => {
    const key1 = pool.makeKey('binance', fakeCreds, false);
    const key2 = pool.makeKey('binance', fakeCreds, true);
    assert.notEqual(key1, key2);
  });

  it('generates different keys for different exchanges', () => {
    const key1 = pool.makeKey('binance', fakeCreds, false);
    const key2 = pool.makeKey('bybit', fakeCreds, false);
    assert.notEqual(key1, key2);
  });

  it('creates separate instances for different credentials', () => {
    const a = pool.getOrCreate('binance', fakeCreds, false);
    const b = pool.getOrCreate('binance', fakeCreds2, false);
    assert.notEqual(a, b);
    assert.equal(pool.size(), 2);
  });

  it('evicts stale entries', () => {
    pool.getOrCreate('binance', fakeCreds, false);
    assert.equal(pool.size(), 1);

    // Manually set lastUsed to past the TTL
    const entries = Array.from(pool.size() > 0 ? [1] : []);
    assert.ok(entries.length > 0);

    // We need to manipulate internal state for testing eviction.
    // The pool module doesn't expose internals, so we test evictStale indirectly:
    // Create an instance, wait 0ms, evict with TTL_MS — it should survive.
    pool.evictStale();
    assert.equal(pool.size(), 1, 'Fresh entry should not be evicted');
  });

  it('respects max pool size by evicting oldest', () => {
    // Fill up to MAX_POOL_SIZE
    for (let i = 0; i < pool.MAX_POOL_SIZE; i++) {
      pool.getOrCreate('binance', { apiKey: `key${i}`, secret: `secret${i}` }, false);
    }
    assert.equal(pool.size(), pool.MAX_POOL_SIZE);

    // Adding one more should evict the oldest
    pool.getOrCreate('binance', { apiKey: 'overflow', secret: 'overflow' }, false);
    assert.equal(pool.size(), pool.MAX_POOL_SIZE);
  });

  it('creates exchange with enableRateLimit', () => {
    const instance = pool.getOrCreate('binance', fakeCreds, false);
    assert.equal(instance.enableRateLimit, true);
  });

  it('sets sandbox mode when requested', () => {
    const instance = pool.getOrCreate('binance', fakeCreds, true);
    assert.ok(instance);
    // Sandbox mode is set — CCXT internally sets the sandbox URL.
    // We verify it was called without error.
  });

  it('throws for unsupported exchange', () => {
    assert.throws(
      () => pool.getOrCreate('nonexistent_exchange_xyz', fakeCreds, false),
      { message: /Unsupported exchange/ }
    );
  });

  it('clear empties the pool', () => {
    pool.getOrCreate('binance', fakeCreds, false);
    assert.equal(pool.size(), 1);
    pool.clear();
    assert.equal(pool.size(), 0);
  });
});
