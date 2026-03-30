import express from "express";
import { createServer } from "http";
import { WebSocketServer } from "ws";
import { ExchangeGateway } from "./gateway";
import { createHandlers } from "./handlers";
import { setupFillStreaming } from "./ws-fills";
import { pskGuard } from "./middleware/psk";

const app = express();
app.use(express.json());
app.use(pskGuard);

// Shared gateway for exchange instance lifecycle
const gateway = new ExchangeGateway();
const handlers = createHandlers(gateway);

// ── Routes (same contract as testudo-ccxt) ──
app.get("/health", handlers.handleHealth);
app.post("/balance", handlers.handleBalance);
app.post("/order", handlers.handleOrder);
app.post("/order/edit", handlers.handleEditOrder);
app.post("/order/cancel", handlers.handleCancelOrder);
app.post("/orders/cancel-all", handlers.handleCancelAllOrders);
app.post("/orders/open", handlers.handleOpenOrders);
app.post("/position", handlers.handlePosition);
app.post("/leverage", handlers.handleLeverage);

const PORT = process.env.PORT || 3100;
const server = createServer(app);
const wss = new WebSocketServer({ server, path: "/ws/orders" });

// CEX-05: Wire fill streaming — subscribe, fill forwarding, cancellation detection
setupFillStreaming(wss, gateway);

// SEC-03: Warn at startup when PSK is missing (all non-health requests will be rejected)
if (!process.env.SIDECAR_PSK) {
  console.warn("WARNING: SIDECAR_PSK not set — all non-health requests will be rejected");
}

server.listen(PORT, () => {
  console.log(`testudo-cex listening on port ${PORT}`);
});

export { app, server, wss, gateway };
