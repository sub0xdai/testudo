/**
 * HTTP route handlers — same endpoint contract as testudo-ccxt.
 * Reads come from safe-cex's in-memory Store (no HTTP round-trip to exchange).
 * Writes use safe-cex methods (placeOrder, cancelOrders, updateOrder, etc.).
 */

import type { Request, Response } from "express";
import type { ExchangeGateway, Credentials } from "./gateway";
import type { ExchangeName } from "safe-cex/dist/types";
import { toExchangeSymbol, toBackendSymbol } from "./symbols";

// ── Helpers ──

/** Convert numeric value to string, preserving precision. Returns null for nullish. */
export function stringify(
  val: number | string | null | undefined
): string | null {
  if (val === null || val === undefined) return null;
  return String(val);
}

export interface Envelope {
  exchangeId: ExchangeName;
  credentials: Credentials;
  sandbox: boolean;
  params: Record<string, any>;
}

/** Extract and validate the request envelope. */
export function parseEnvelope(body: any): Envelope {
  const { exchange_id, credentials, sandbox, params } = body;
  if (!exchange_id) throw new Error("Missing exchange_id");
  if (!credentials?.apiKey || !credentials?.secret) {
    throw new Error("Missing or incomplete credentials");
  }
  return {
    exchangeId: exchange_id as ExchangeName,
    credentials: {
      key: credentials.apiKey,
      secret: credentials.secret,
      applicationId: credentials.applicationId,
      passphrase: credentials.password,
    },
    sandbox: Boolean(sandbox),
    params: params || {},
  };
}

/** Extract the most useful error message, including from Axios errors. */
function extractErrorMessage(err: any): string {
  // Axios errors: dig into response.data for exchange-specific messages
  if (err?.isAxiosError && err?.response?.data) {
    const data = err.response.data;
    const exchangeMsg = data.msg || data.message || data.error || JSON.stringify(data);
    const status = err.response.status;
    return `Exchange API error ${status}: ${exchangeMsg}`;
  }
  // safe-cex sometimes wraps errors with a message property
  if (err?.message) return err.message;
  return String(err);
}

/** Map errors to HTTP status codes matching CexClientError enum. */
export function mapError(err: any): {
  status: number;
  body: { error: string; code: string };
} {
  const msg = extractErrorMessage(err);
  const lower = msg.toLowerCase();

  // Binance error codes
  const axiosCode = err?.response?.data?.code;
  if (axiosCode === -2014 || axiosCode === -2015)
    return { status: 401, body: { error: msg, code: "AuthenticationError" } };

  if (lower.includes("401") || lower.includes("auth") || lower.includes("api-key") || lower.includes("invalid key"))
    return { status: 401, body: { error: msg, code: "AuthenticationError" } };
  if (lower.includes("insufficient") || lower.includes("margin"))
    return { status: 402, body: { error: msg, code: "InsufficientFunds" } };
  if (lower.includes("not found"))
    return { status: 404, body: { error: msg, code: "OrderNotFound" } };
  if (lower.includes("rate") || lower.includes("429"))
    return { status: 429, body: { error: msg, code: "RateLimitExceeded" } };
  return { status: 502, body: { error: msg, code: "ExchangeError" } };
}

// ── Handler factory ──

export function createHandlers(gateway: ExchangeGateway) {
  /** Get exchange instance + params from request body. */
  async function getExchange(body: any) {
    const envelope = parseEnvelope(body);
    const exchange = await gateway.getOrCreate(
      envelope.exchangeId,
      envelope.credentials,
      envelope.sandbox,
      () => {} // CEX-05 will wire fill streaming
    );
    return { exchange, params: envelope.params };
  }

  // ── GET /health ──

  async function handleHealth(_req: Request, res: Response) {
    try {
      return res.json({ ok: true });
    } catch {
      return res.status(503).json({ ok: false });
    }
  }

  // ── POST /balance — reads from Store (FR-1) ──

  async function handleBalance(req: Request, res: Response) {
    try {
      const { exchange } = await getExchange(req.body);
      const balance = exchange.store.balance;

      // safe-cex balance is a single object for the futures account.
      // total = wallet balance, upnl = unrealized PnL.
      // Report equity (total + upnl) as "total" — matches what the exchange UI shows.
      const equity = (balance.total || 0) + (balance.upnl || 0);
      const result = [
        {
          asset: "USDT",
          total: stringify(equity),
          free: stringify(balance.free),
          used: stringify(balance.used),
        },
      ];

      return res.json(result);
    } catch (err) {
      const mapped = mapError(err);
      return res.status(mapped.status).json(mapped.body);
    }
  }

  // ── POST /order — placeOrder with bracket support (FR-2, FR-12) ──

  async function handleOrder(req: Request, res: Response) {
    try {
      const { exchange, params } = await getExchange(req.body);
      const {
        symbol,
        type,
        side,
        amount,
        price,
        leverage,
        reduceOnly,
        clientOrderId,
        stopLoss,
        takeProfit,
      } = params;

      // CEX-07: Convert backend symbol to exchange format
      const exchSymbol = toExchangeSymbol(symbol);

      // Set leverage if provided (graceful fallback — FR-16)
      if (leverage && leverage > 0) {
        try {
          await exchange.setLeverage(exchSymbol, leverage);
        } catch (levErr: any) {
          console.error("[LEVERAGE ERROR]", {
            leverage,
            symbol: exchSymbol,
            error: levErr?.message,
          });
        }
      }

      // Build placeOrder options
      const orderOpts: any = {
        symbol: exchSymbol,
        type: type || "limit",
        side,
        amount: Number(amount),
        reduceOnly: reduceOnly || false,
      };
      if (price != null) orderOpts.price = Number(price);

      // Bracket order SL/TP (FR-12)
      if (stopLoss?.triggerPrice)
        orderOpts.stopLoss = Number(stopLoss.triggerPrice);
      if (takeProfit?.triggerPrice)
        orderOpts.takeProfit = Number(takeProfit.triggerPrice);

      const orderIds: string[] = await exchange.placeOrder(orderOpts);

      // Bracket IDs: WOO/Binance return [entry, sl, tp] — happy path below.
      // Bybit attaches SL/TP as conditional fields on the entry order and
      // returns only [entry]; the conditional orders appear in store.orders
      // via the WS stream within a few hundred ms. Fall back to matching
      // them by triggerPrice when the positional slots are empty.
      let stopLossOrderId = stringify(orderIds[1]);
      let takeProfitOrderId = stringify(orderIds[2]);

      async function resolveConditionalId(
        triggerPrice: number,
        role: "SL" | "TP"
      ): Promise<string | null> {
        const trig = Number(triggerPrice);
        if (!Number.isFinite(trig)) return null;
        const tolerance = Math.max(1e-6, Math.abs(trig) * 1e-5);
        // Poll the store briefly — WS sync is usually sub-second.
        for (let attempt = 0; attempt < 10; attempt++) {
          const match = exchange.store.orders.find((o: any) => {
            if (o.symbol !== exchSymbol) return false;
            const t = Number(o.stopPrice ?? o.triggerPrice ?? o.price);
            if (!Number.isFinite(t)) return false;
            return Math.abs(t - trig) <= tolerance;
          });
          if (match?.id) return String(match.id);
          await new Promise((resolve) => setTimeout(resolve, 150));
        }
        console.warn(
          `[bracket-fallback] ${role} triggerPrice=${trig} symbol=${exchSymbol} not found in store after 1.5s — fill detection degraded`
        );
        return null;
      }

      if (!stopLossOrderId && stopLoss?.triggerPrice != null) {
        stopLossOrderId = await resolveConditionalId(
          Number(stopLoss.triggerPrice),
          "SL"
        );
      }
      if (!takeProfitOrderId && takeProfit?.triggerPrice != null) {
        takeProfitOrderId = await resolveConditionalId(
          Number(takeProfit.triggerPrice),
          "TP"
        );
      }

      // Map to SidecarOrderResponse shape (FR-10: all numerics as strings)
      return res.json({
        id: stringify(orderIds[0]),
        clientOrderId: clientOrderId || null,
        status: "open",
        symbol,
        side,
        type: type || "limit",
        amount: stringify(amount),
        filled: "0",
        remaining: stringify(amount),
        average: null,
        price: stringify(price),
        stopLossOrderId,
        takeProfitOrderId,
      });
    } catch (err) {
      const mapped = mapError(err);
      return res.status(mapped.status).json(mapped.body);
    }
  }

  // ── POST /order/edit — updateOrder (FR-8) ──

  async function handleEditOrder(req: Request, res: Response) {
    try {
      const { exchange, params } = await getExchange(req.body);
      const { orderId, symbol, type, side, amount, price } = params;

      // Find the order in the Store
      const storeOrder = exchange.store.orders.find(
        (o: any) => o.id === orderId
      );
      if (!storeOrder) {
        return res.status(404).json({
          error: `Order not found: ${orderId}`,
          code: "OrderNotFound",
        });
      }

      // Build update — safe-cex accepts {amount} or {price}
      const update: any = {};
      if (amount != null) update.amount = Number(amount);
      if (price != null) update.price = Number(price);

      await exchange.updateOrder({ order: storeOrder, update });

      return res.json({
        id: stringify(orderId),
        status: storeOrder.status || "open",
        symbol: symbol || toBackendSymbol(storeOrder.symbol, exchange.store.markets),
        side: side || storeOrder.side,
        type: type || storeOrder.type,
        amount: stringify(amount ?? storeOrder.amount),
        filled: stringify(storeOrder.filled),
        remaining: stringify(storeOrder.remaining),
        average: null,
        price: stringify(price ?? storeOrder.price),
      });
    } catch (err) {
      const mapped = mapError(err);
      return res.status(mapped.status).json(mapped.body);
    }
  }

  // ── POST /order/cancel — cancelOrders (FR-3) ──

  async function handleCancelOrder(req: Request, res: Response) {
    try {
      const { exchange, params } = await getExchange(req.body);
      const { orderId, symbol } = params;
      const exchSymbol = toExchangeSymbol(symbol);

      // Find the order in the Store to pass to cancelOrders
      const storeOrder = exchange.store.orders.find(
        (o: any) => o.id === orderId
      );
      if (storeOrder) {
        await exchange.cancelOrders([storeOrder]);
      } else {
        // Order may not be in store — pass minimal object
        await exchange.cancelOrders([{ id: orderId, symbol: exchSymbol } as any]);
      }

      return res.json({ success: true });
    } catch (err) {
      const mapped = mapError(err);
      return res.status(mapped.status).json(mapped.body);
    }
  }

  // ── POST /orders/cancel-all — cancelSymbolOrders (FR-4) ──

  async function handleCancelAllOrders(req: Request, res: Response) {
    try {
      const { exchange, params } = await getExchange(req.body);
      const { symbol } = params;

      await exchange.cancelSymbolOrders(toExchangeSymbol(symbol));

      return res.json({ success: true, cancelled: 0 });
    } catch (err) {
      const mapped = mapError(err);
      return res.status(mapped.status).json(mapped.body);
    }
  }

  // ── POST /position — reads from Store (FR-5) ──

  async function handlePosition(req: Request, res: Response) {
    try {
      const { exchange, params } = await getExchange(req.body);
      const { symbol } = params;

      let positions = exchange.store.positions;
      if (symbol) {
        const exchSymbol = toExchangeSymbol(symbol);
        positions = positions.filter((p: any) => p.symbol === exchSymbol);
      }

      const result = positions.map((pos: any) => ({
        symbol: toBackendSymbol(pos.symbol, exchange.store.markets),
        side: pos.side,
        contracts: stringify(pos.contracts),
        entryPrice: stringify(pos.entryPrice),
        unrealizedPnl: stringify(pos.unrealizedPnl),
        leverage: pos.leverage != null ? stringify(pos.leverage) : undefined,
      }));

      return res.json(result);
    } catch (err) {
      const mapped = mapError(err);
      return res.status(mapped.status).json(mapped.body);
    }
  }

  // ── POST /leverage — setLeverage (FR-6) ──

  async function handleLeverage(req: Request, res: Response) {
    try {
      const { exchange, params } = await getExchange(req.body);
      const { leverage, symbol } = params;

      // safe-cex: setLeverage(symbol, leverage) — note param order differs from CCXT
      await exchange.setLeverage(toExchangeSymbol(symbol), leverage);

      return res.json({ success: true });
    } catch (err) {
      const mapped = mapError(err);
      return res.status(mapped.status).json(mapped.body);
    }
  }

  // ── POST /orders/open — reads from Store (FR-7) ──

  async function handleOpenOrders(req: Request, res: Response) {
    try {
      const { exchange, params } = await getExchange(req.body);
      const { symbol } = params;

      let orders = exchange.store.orders;
      if (symbol) {
        const exchSymbol = toExchangeSymbol(symbol);
        orders = orders.filter((o: any) => o.symbol === exchSymbol);
      }

      const result = orders.map((o: any) => ({
        id: stringify(o.id),
        clientOrderId: null,
        symbol: toBackendSymbol(o.symbol, exchange.store.markets),
        status: o.status,
        side: o.side,
        type: o.type,
        price: stringify(o.price),
        stopPrice: null,
        amount: stringify(o.amount),
        filled: stringify(o.filled),
        remaining: stringify(o.remaining),
        timestamp: null,
      }));

      return res.json(result);
    } catch (err) {
      const mapped = mapError(err);
      return res.status(mapped.status).json(mapped.body);
    }
  }

  return {
    handleHealth,
    handleBalance,
    handleOrder,
    handleEditOrder,
    handleCancelOrder,
    handleCancelAllOrders,
    handlePosition,
    handleLeverage,
    handleOpenOrders,
  };
}
