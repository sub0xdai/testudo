'use strict';

const http = require('node:http');
const express = require('express');
const ccxt = require('ccxt');
const { WebSocketServer } = require('ws');
const pool = require('./pool');
const {
  handleBalance,
  handleOrder,
  handleEditOrder,
  handleCancelOrder,
  handlePosition,
  handleLeverage,
} = require('./handlers');
const { handleOrdersConnection } = require('./ws-orders');

const PORT = parseInt(process.env.CCXT_PORT, 10) || 3100;
const HOST = '127.0.0.1';

const app = express();

app.use(express.json());

// Request logging middleware - logs method, path, status. NEVER logs body.
app.use((req, res, next) => {
  const start = Date.now();
  res.on('finish', () => {
    const duration = Date.now() - start;
    console.log(`${req.method} ${req.path} ${res.statusCode} ${duration}ms`);
  });
  next();
});

// --- Routes ---

app.get('/health', (_req, res) => {
  res.json({
    ok: true,
    poolSize: pool.size(),
    uptime: process.uptime(),
  });
});

app.get('/exchanges', (_req, res) => {
  res.json(ccxt.exchanges);
});

app.post('/balance', handleBalance);
app.post('/order', handleOrder);
app.post('/order/edit', handleEditOrder);
app.post('/order/cancel', handleCancelOrder);
app.post('/position', handlePosition);
app.post('/leverage', handleLeverage);

// Start eviction timer
pool.startEviction();

// EXT-22: Create HTTP server and attach WebSocket server for order streaming
const server = http.createServer(app);
const wss = new WebSocketServer({ server, path: '/ws/orders' });

wss.on('connection', (ws) => {
  console.log('WS client connected to /ws/orders');
  handleOrdersConnection(ws);
});

// Only start listening if this file is run directly (not required by tests)
if (require.main === module) {
  server.listen(PORT, HOST, () => {
    console.log(`CCXT sidecar listening on ${HOST}:${PORT} (HTTP + WS)`);
  });
}

module.exports = { app, server, wss };
