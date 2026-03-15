/**
 * WebSocket fill streaming — CEX-05.
 *
 * Forwards safe-cex fill events and detects order cancellations via Store
 * diffing, sending `order_update` events to the connected Rust backend.
 *
 * Event shape matches the `OrderUpdateEvent` struct in ccxt_client.rs exactly:
 * numeric fields are JSON numbers (f64/i64), not strings.
 */

import type { WebSocketServer, WebSocket } from "ws";
import type { ExchangeGateway } from "./gateway";
import type { ExchangeName, OrderFillEvent } from "safe-cex/dist/types";

/** Order snapshot for diffing (matches safe-cex Order shape). */
export interface OrderSnapshot {
  id: string;
  symbol: string;
  side: string;
  price: number;
  amount: number;
  filled: number;
  remaining: number;
}

/** Wire shape sent over WebSocket — matches Rust OrderUpdateEvent. */
export interface OrderUpdatePayload {
  id: string;
  symbol: string;
  status: "closed" | "canceled";
  side: string;
  price: number;
  amount: number;
  filled: number;
  remaining: number;
  average: number;
  timestamp: number;
}

/** Subscribe message from Rust backend (matches WsSubscribeMessage). */
export interface SubscribeMessage {
  action: string;
  exchange_id: string;
  credentials: {
    apiKey: string;
    secret: string;
    password?: string;
  };
  sandbox: boolean;
  symbols: string[];
}

/** Send an order_update event to the WebSocket client. */
export function sendOrderUpdate(ws: WebSocket, data: OrderUpdatePayload): void {
  if (ws.readyState === ws.OPEN) {
    ws.send(JSON.stringify({ event: "order_update", data }));
  }
}

/**
 * Match pending fills to pending removals and emit the appropriate events.
 *
 * Strategy:
 * - A removal matched to a fill = full fill → status "closed"
 * - A removal with no matching fill = cancellation → status "canceled"
 * - A fill with no matching removal = partial fill → ignored (Rust backend
 *   only processes "closed" and "canceled")
 *
 * This handles both event orderings:
 * - Regular orders: update (removal) fires before fill
 * - Algo orders: fill fires before update (removal)
 */
export function processPending(
  ws: WebSocket,
  pendingFills: OrderFillEvent[],
  pendingRemovals: Map<string, OrderSnapshot>
): void {
  // Match fills to removals
  for (const fill of pendingFills) {
    for (const [id, order] of pendingRemovals) {
      if (order.symbol === fill.symbol && order.side === fill.side) {
        sendOrderUpdate(ws, {
          id,
          symbol: fill.symbol,
          status: "closed",
          side: fill.side,
          price: order.price,
          amount: order.amount,
          filled: fill.amount,
          remaining: 0,
          average: fill.price,
          timestamp: Date.now(),
        });
        pendingRemovals.delete(id);
        break;
      }
    }
  }

  // Remaining unmatched removals are cancellations
  for (const [id, order] of pendingRemovals) {
    sendOrderUpdate(ws, {
      id,
      symbol: order.symbol,
      status: "canceled",
      side: order.side,
      price: order.price,
      amount: order.amount,
      filled: order.filled || 0,
      remaining: order.remaining || order.amount,
      average: order.price,
      timestamp: Date.now(),
    });
  }

  pendingFills.length = 0;
  pendingRemovals.clear();
}

/** Extract minimal order snapshot from a safe-cex Order. */
export function snapshotOrder(o: any): OrderSnapshot {
  return {
    id: o.id,
    symbol: o.symbol,
    side: o.side,
    price: o.price,
    amount: o.amount,
    filled: o.filled ?? 0,
    remaining: o.remaining ?? o.amount,
  };
}

/**
 * Set up WebSocket fill streaming on the given WSS.
 *
 * When a Rust backend connects to /ws/orders and sends a subscribe message,
 * this wires safe-cex fill + update events and forwards them as order_update
 * messages matching the OrderUpdateEvent struct.
 */
export function setupFillStreaming(
  wss: WebSocketServer,
  gateway: ExchangeGateway
): void {
  wss.on("connection", (ws) => {
    console.log("[WS] Client connected");

    let cleanup: (() => void) | null = null;

    ws.on("message", async (raw) => {
      try {
        const msg: SubscribeMessage = JSON.parse(String(raw));
        if (msg.action !== "subscribe") return;

        const credentials = {
          key: msg.credentials.apiKey,
          secret: msg.credentials.secret,
          passphrase: msg.credentials.password,
        };

        const exchange = await gateway.getOrCreate(
          msg.exchange_id as ExchangeName,
          credentials,
          msg.sandbox,
          () => {} // Fill handled by the listener below
        );

        // Initialize known orders from current Store state
        let knownOrders = new Map<string, OrderSnapshot>(
          exchange.store.orders.map((o: any) => [o.id, snapshotOrder(o)])
        );

        const pendingFills: OrderFillEvent[] = [];
        const pendingRemovals = new Map<string, OrderSnapshot>();
        let flushScheduled = false;

        function scheduleFlush() {
          if (!flushScheduled) {
            flushScheduled = true;
            queueMicrotask(() => {
              flushScheduled = false;
              processPending(ws, pendingFills, pendingRemovals);
            });
          }
        }

        // Fill listener — accumulates fills for microtask matching
        const onFill = (fill: OrderFillEvent) => {
          pendingFills.push(fill);
          scheduleFlush();
        };

        // Update listener — detects order removals (fills + cancellations)
        const onUpdate = (data: { orders: any[] }) => {
          const currentIds = new Set<string>();
          const currentOrders = new Map<string, OrderSnapshot>();

          for (const o of data.orders) {
            currentIds.add(o.id);
            currentOrders.set(o.id, snapshotOrder(o));
          }

          for (const [id, order] of knownOrders) {
            if (!currentIds.has(id)) {
              pendingRemovals.set(id, order);
            }
          }

          knownOrders = currentOrders;
          if (pendingRemovals.size > 0) {
            scheduleFlush();
          }
        };

        exchange.on("fill", onFill);
        exchange.on("update", onUpdate);

        cleanup = () => {
          exchange.off("fill", onFill);
          exchange.off("update", onUpdate);
        };

        ws.send(
          JSON.stringify({
            event: "subscribed",
            message: "Streaming order updates",
          })
        );

        console.log(`[WS] Subscribed to ${msg.exchange_id} fills`);
      } catch (err) {
        console.error("[WS] Subscribe error:", err);
        ws.send(JSON.stringify({ event: "error", message: String(err) }));
      }
    });

    ws.on("close", () => {
      console.log("[WS] Client disconnected");
      if (cleanup) cleanup();
    });
  });
}
