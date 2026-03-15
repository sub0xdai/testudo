import express from "express";
import { createServer } from "http";
import { WebSocketServer } from "ws";
import { ExchangeGateway } from "./gateway";
import { createHandlers } from "./handlers";
import { setupFillStreaming } from "./ws-fills";

const app = express();
app.use(express.json());

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

server.listen(PORT, () => {
  console.log(`testudo-cex listening on port ${PORT}`);
});

export { app, server, wss, gateway };
