'use strict';

const pool = require('./pool');
const { mapError } = require('./errors');

/**
 * Convert a numeric value to a string, preserving precision.
 * Returns null/undefined as-is for optional fields.
 * @param {number|string|null|undefined} val
 * @returns {string|null|undefined}
 */
function stringify(val) {
  if (val === null || val === undefined) return null;
  return String(val);
}

/**
 * Extract the envelope fields from a request body.
 * @param {object} body
 * @returns {{ exchangeId: string, credentials: object, sandbox: boolean, params: object }}
 */
function parseEnvelope(body) {
  const { exchange_id, credentials, sandbox, params } = body;

  if (!exchange_id) throw new Error('Missing exchange_id');
  if (!credentials || !credentials.apiKey || !credentials.secret) {
    throw new Error('Missing or incomplete credentials');
  }

  return {
    exchangeId: exchange_id,
    credentials,
    sandbox: Boolean(sandbox),
    params: params || {},
  };
}

/**
 * Get exchange instance from pool using request envelope.
 * @param {object} body
 * @returns {{ exchange: object, params: object }}
 */
function getExchangeAndParams(body) {
  const { exchangeId, credentials, sandbox, params } = parseEnvelope(body);
  const exchange = pool.getOrCreate(exchangeId, credentials, sandbox);
  return { exchange, params };
}

/**
 * POST /balance
 */
async function handleBalance(req, res) {
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const balance = await exchange.fetchBalance({ type: params.type || 'future' });

    const result = [];
    const currencies = balance.info ? Object.keys(balance.total || {}) : [];

    for (const asset of currencies) {
      const total = balance.total[asset];
      const free = balance.free[asset];
      const used = balance.used[asset];

      if (total === undefined && free === undefined && used === undefined) continue;

      result.push({
        asset,
        total: stringify(total),
        free: stringify(free),
        used: stringify(used),
      });
    }

    const usdt = result.find(r => r.asset === 'USDT');
    if (usdt) console.log('[BALANCE USDT]', usdt);
    res.json(result);
  } catch (err) {
    const mapped = mapError(err);
    res.status(mapped.status).json(mapped.body);
  }
}

/**
 * POST /order
 */
async function handleOrder(req, res) {
  let orderDetail = {};
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const { symbol, type, side, amount, price, stopPrice, leverage,
            reduceOnly, clientOrderId, stopLoss, takeProfit } = params;

    if (leverage && leverage > 0) {
      try {
        await exchange.setLeverage(leverage, symbol);
        console.log('[LEVERAGE OK]', { leverage, symbol });
      } catch (levErr) {
        console.error('[LEVERAGE ERROR]', { leverage, symbol, error: levErr.message });
        // Continue — some exchanges don't support setLeverage or already have it set
      }
    }

    const orderParams = {};
    if (stopPrice !== undefined && stopPrice !== null) {
      orderParams.stopPrice = stopPrice;
    }
    if (reduceOnly) {
      orderParams.reduceOnly = true;
    }
    // EXT-24 FR-5: Stamp clientOrderId for defense-in-depth identification
    if (clientOrderId) {
      orderParams.clientOrderId = clientOrderId;
    }
    // EXT-31: Bracket order — attach SL/TP to entry (exchange activates on fill)
    if (stopLoss && stopLoss.triggerPrice) {
      orderParams.stopLoss = { triggerPrice: stopLoss.triggerPrice };
    }
    if (takeProfit && takeProfit.triggerPrice) {
      orderParams.takeProfit = { triggerPrice: takeProfit.triggerPrice };
    }

    orderDetail = { symbol, type, side, amount, price, leverage, reduceOnly, orderParams };
    console.log('[ORDER REQ]', orderDetail);
    const order = await exchange.createOrder(symbol, type, side, amount, price, orderParams);

    res.json({
      id: stringify(order.id),
      clientOrderId: order.clientOrderId || null,
      status: order.status,
      symbol: order.symbol,
      side: order.side,
      type: order.type,
      amount: stringify(order.amount),
      filled: stringify(order.filled),
      remaining: stringify(order.remaining),
      average: stringify(order.average),
      price: stringify(order.price),
      // EXT-31: Bracket order child IDs (if exchange returns them)
      stopLossOrderId: order.info?.stopLossOrderId || null,
      takeProfitOrderId: order.info?.takeProfitOrderId || null,
    });
  } catch (err) {
    console.error('[ORDER ERROR]', {
      ...orderDetail,
      error: err.constructor.name,
      message: err.message,
    });
    const mapped = mapError(err);
    res.status(mapped.status).json(mapped.body);
  }
}

/**
 * POST /order/edit
 */
async function handleEditOrder(req, res) {
  let editDetail = {};
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const { orderId, symbol, type, side, amount, price } = params;

    editDetail = { orderId, symbol, type, side, amount, price };
    console.log('[EDIT REQ]', editDetail);
    const order = await exchange.editOrder(orderId, symbol, type, side, amount, price);

    res.json({
      id: stringify(order.id),
      status: order.status,
      symbol: order.symbol,
      side: order.side,
      type: order.type,
      amount: stringify(order.amount),
      filled: stringify(order.filled),
      remaining: stringify(order.remaining),
      average: stringify(order.average),
      price: stringify(order.price),
    });
  } catch (err) {
    console.error('[EDIT ERROR]', {
      ...editDetail,
      error: err.constructor.name,
      message: err.message,
    });
    const mapped = mapError(err);
    res.status(mapped.status).json(mapped.body);
  }
}

/**
 * POST /order/cancel
 */
async function handleCancelOrder(req, res) {
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const { orderId, symbol } = params;

    await exchange.cancelOrder(orderId, symbol);

    res.json({ success: true });
  } catch (err) {
    const mapped = mapError(err);
    res.status(mapped.status).json(mapped.body);
  }
}

/**
 * POST /position
 */
async function handlePosition(req, res) {
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const { symbol } = params;

    const positions = symbol
      ? await exchange.fetchPositions([symbol])
      : await exchange.fetchPositions();

    const result = positions.map((pos) => ({
      symbol: pos.symbol,
      side: pos.side,
      contracts: stringify(pos.contracts),
      entryPrice: stringify(pos.entryPrice),
      unrealizedPnl: stringify(pos.unrealizedPnl),
    }));

    res.json(result);
  } catch (err) {
    const mapped = mapError(err);
    res.status(mapped.status).json(mapped.body);
  }
}

/**
 * POST /leverage
 */
async function handleLeverage(req, res) {
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const { leverage, symbol } = params;

    await exchange.setLeverage(leverage, symbol);

    res.json({ success: true });
  } catch (err) {
    const mapped = mapError(err);
    res.status(mapped.status).json(mapped.body);
  }
}

/**
 * POST /orders/open
 * EXT-24 FR-3: Fetch open orders for a symbol (or all) from the exchange.
 */
async function handleOpenOrders(req, res) {
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const { symbol } = params;

    const orders = await exchange.fetchOpenOrders(symbol || undefined);

    const result = orders.map((o) => ({
      id: stringify(o.id),
      clientOrderId: o.clientOrderId || null,
      symbol: o.symbol,
      status: o.status,
      side: o.side,
      type: o.type,
      price: stringify(o.price),
      stopPrice: stringify(o.stopPrice),
      amount: stringify(o.amount),
      filled: stringify(o.filled),
      remaining: stringify(o.remaining),
      timestamp: o.timestamp,
    }));

    res.json(result);
  } catch (err) {
    const mapped = mapError(err);
    res.status(mapped.status).json(mapped.body);
  }
}

/**
 * POST /orders/cancel-all
 * Defense-in-depth: cancel ALL open orders for a symbol in one API call.
 */
async function handleCancelAllOrders(req, res) {
  try {
    const { exchange, params } = getExchangeAndParams(req.body);
    const { symbol } = params;

    const result = await exchange.cancelAllOrders(symbol || undefined);

    res.json({ success: true, cancelled: Array.isArray(result) ? result.length : 0 });
  } catch (err) {
    const mapped = mapError(err);
    res.status(mapped.status).json(mapped.body);
  }
}

module.exports = {
  handleBalance,
  handleOrder,
  handleEditOrder,
  handleCancelOrder,
  handleCancelAllOrders,
  handlePosition,
  handleLeverage,
  handleOpenOrders,
  stringify,
};
