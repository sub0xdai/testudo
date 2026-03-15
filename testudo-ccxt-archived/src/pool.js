'use strict';

const crypto = require('node:crypto');
const ccxt = require('ccxt');

const MAX_POOL_SIZE = 100;
const TTL_MS = 30 * 60 * 1000; // 30 minutes
const EVICTION_INTERVAL_MS = 5 * 60 * 1000; // 5 minutes

/** @type {Map<string, { instance: object, lastUsed: number, key: string }>} */
const pool = new Map();

let evictionTimer = null;

/**
 * Generate a cache key from exchange ID, API key, and sandbox flag.
 * @param {string} exchangeId
 * @param {{ apiKey: string, secret: string, password?: string }} credentials
 * @param {boolean} sandbox
 * @returns {string}
 */
function makeKey(exchangeId, credentials, sandbox) {
  const raw = `${exchangeId}${credentials.apiKey}${sandbox}`;
  return crypto.createHash('sha256').update(raw).digest('hex');
}

/**
 * Get or create a CCXT exchange instance from the pool.
 * @param {string} exchangeId - CCXT exchange ID (e.g. 'binance', 'bybit')
 * @param {{ apiKey: string, secret: string, password?: string }} credentials
 * @param {boolean} sandbox - Whether to use sandbox/testnet mode
 * @returns {object} CCXT exchange instance
 */
function getOrCreate(exchangeId, credentials, sandbox) {
  const key = makeKey(exchangeId, credentials, sandbox);

  const cached = pool.get(key);
  if (cached) {
    cached.lastUsed = Date.now();
    return cached.instance;
  }

  if (pool.size >= MAX_POOL_SIZE) {
    evictOldest();
  }

  const ExchangeClass = ccxt[exchangeId];
  if (!ExchangeClass) {
    throw new Error(`Unsupported exchange: ${exchangeId}`);
  }

  const instance = new ExchangeClass({
    apiKey: credentials.apiKey,
    secret: credentials.secret,
    password: credentials.password,
    enableRateLimit: true,
  });

  if (sandbox) {
    instance.setSandboxMode(true);
  }

  pool.set(key, { instance, lastUsed: Date.now(), key });

  return instance;
}

/**
 * Evict pool entries that have been inactive beyond the TTL.
 */
function evictStale() {
  const now = Date.now();
  for (const [key, entry] of pool) {
    if (now - entry.lastUsed > TTL_MS) {
      pool.delete(key);
    }
  }
}

/**
 * Evict the oldest entry in the pool (by lastUsed time).
 */
function evictOldest() {
  let oldestKey = null;
  let oldestTime = Infinity;

  for (const [key, entry] of pool) {
    if (entry.lastUsed < oldestTime) {
      oldestTime = entry.lastUsed;
      oldestKey = key;
    }
  }

  if (oldestKey) {
    pool.delete(oldestKey);
  }
}

/**
 * Start the periodic eviction timer.
 */
function startEviction() {
  if (evictionTimer) return;
  evictionTimer = setInterval(evictStale, EVICTION_INTERVAL_MS);
  evictionTimer.unref();
}

/**
 * Stop the periodic eviction timer.
 */
function stopEviction() {
  if (evictionTimer) {
    clearInterval(evictionTimer);
    evictionTimer = null;
  }
}

/**
 * Get current pool size.
 * @returns {number}
 */
function size() {
  return pool.size;
}

/**
 * Clear all entries from the pool.
 */
function clear() {
  pool.clear();
}

module.exports = {
  getOrCreate,
  evictStale,
  size,
  clear,
  startEviction,
  stopEviction,
  makeKey,
  MAX_POOL_SIZE,
  TTL_MS,
};
