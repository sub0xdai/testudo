'use strict';

const pool = require('./pool');

/**
 * Handle a WebSocket connection for order streaming via CCXT watchOrders().
 *
 * Protocol:
 * 1. Client sends JSON subscribe message:
 *    { "action": "subscribe", "exchange_id": "...", "credentials": {...}, "sandbox": bool, "symbols": [...] }
 * 2. Server starts watchOrders() loops and pushes order_update events:
 *    { "event": "order_update", "data": { id, symbol, status, side, price, amount, filled, remaining, average, timestamp } }
 * 3. Server sends error/status events:
 *    { "event": "error", "message": "..." }
 *    { "event": "subscribed", "symbols": [...] }
 *
 * @param {import('ws').WebSocket} ws
 */
function handleOrdersConnection(ws) {
  let abortController = null;

  ws.on('message', (raw) => {
    let msg;
    try {
      msg = JSON.parse(raw.toString());
    } catch {
      ws.send(JSON.stringify({ event: 'error', message: 'Invalid JSON' }));
      return;
    }

    if (msg.action === 'subscribe') {
      handleSubscribe(ws, msg, abortController).then((ac) => {
        abortController = ac;
      });
    }
  });

  ws.on('close', () => {
    if (abortController) {
      abortController.abort();
    }
  });

  ws.on('error', () => {
    if (abortController) {
      abortController.abort();
    }
  });
}

/**
 * Start watchOrders() loops for the requested symbols.
 * @param {import('ws').WebSocket} ws
 * @param {object} msg - Subscribe message
 * @param {AbortController|null} prevController - Previous abort controller to cancel
 * @returns {Promise<AbortController>}
 */
async function handleSubscribe(ws, msg, prevController) {
  // Cancel any previous subscription
  if (prevController) {
    prevController.abort();
  }

  const { exchange_id, credentials, sandbox, symbols } = msg;

  if (!exchange_id || !credentials || !credentials.apiKey || !credentials.secret) {
    ws.send(JSON.stringify({ event: 'error', message: 'Missing exchange_id or credentials' }));
    return null;
  }

  if (!Array.isArray(symbols) || symbols.length === 0) {
    ws.send(JSON.stringify({ event: 'error', message: 'symbols must be a non-empty array' }));
    return null;
  }

  let exchange;
  try {
    exchange = pool.getOrCreate(exchange_id, credentials, Boolean(sandbox));
  } catch (err) {
    ws.send(JSON.stringify({ event: 'error', message: err.message }));
    return null;
  }

  const ac = new AbortController();

  ws.send(JSON.stringify({ event: 'subscribed', symbols }));

  // Start a watchOrders loop for each symbol
  for (const symbol of symbols) {
    watchOrdersLoop(ws, exchange, symbol, ac.signal);
  }

  return ac;
}

/**
 * Continuously watch orders for a symbol and push updates.
 * @param {import('ws').WebSocket} ws
 * @param {object} exchange - CCXT exchange instance
 * @param {string} symbol - Trading pair symbol
 * @param {AbortSignal} signal - Abort signal to stop the loop
 */
async function watchOrdersLoop(ws, exchange, symbol, signal) {
  while (!signal.aborted) {
    try {
      if (typeof exchange.watchOrders !== 'function' || !exchange.has['watchOrders']) {
        ws.send(JSON.stringify({
          event: 'error',
          message: `watchOrders not supported by ${exchange.id}`,
        }));
        return;
      }

      const orders = await exchange.watchOrders(symbol);

      if (signal.aborted) return;

      for (const order of orders) {
        if (signal.aborted) return;
        if (ws.readyState !== 1) return; // OPEN

        ws.send(JSON.stringify({
          event: 'order_update',
          data: {
            id: String(order.id),
            symbol: order.symbol,
            status: order.status,
            side: order.side,
            price: order.price,
            amount: order.amount,
            filled: order.filled,
            remaining: order.remaining,
            average: order.average,
            timestamp: order.timestamp,
          },
        }));
      }
    } catch (err) {
      if (signal.aborted) return;
      if (ws.readyState !== 1) return;

      const msg = err.message || '';
      const notSupported = msg.includes('not supported') || err.constructor?.name === 'NotSupported';

      ws.send(JSON.stringify({
        event: notSupported ? 'unsupported' : 'error',
        message: `watchOrders ${notSupported ? 'not supported' : 'error'} for ${symbol}: ${msg}`,
      }));

      // Permanent errors — close connection with 4000 (unsupported) code
      if (notSupported) {
        ws.close(4000, 'watchOrders not supported');
        return;
      }

      // Transient errors — pause before retry
      await new Promise((r) => setTimeout(r, 1000));
    }
  }
}

module.exports = { handleOrdersConnection, handleSubscribe, watchOrdersLoop };
