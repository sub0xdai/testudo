import express from "express";
import { createServer } from "http";
import { WebSocketServer } from "ws";

const app = express();
app.use(express.json());

app.get("/health", (_req, res) => {
  res.json({ ok: true });
});

const PORT = process.env.PORT || 3100;
const server = createServer(app);
const wss = new WebSocketServer({ server, path: "/ws/orders" });

wss.on("connection", (ws) => {
  console.log("WebSocket client connected");
  ws.on("close", () => console.log("WebSocket client disconnected"));
});

server.listen(PORT, () => {
  console.log(`testudo-cex listening on port ${PORT}`);
});

export { app, server, wss };
