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
  handleCancelAllOrders,
  handlePosition,
  handleLeverage,
  handleOpenOrders,
  handleTrades,
} = require('./handlers');
const { handleOrdersConnection } = require('./ws-orders');
const { register, sidecarRequestsTotal, sidecarLatencySeconds, sidecarPoolSize } = require('./metrics');

const PORT = parseInt(process.env.CCXT_PORT, 10) || 3100;
const HOST = '127.0.0.1';

const app = express();

app.use(express.json());

// Request logging + metrics middleware. NEVER logs body.
app.use((req, res, next) => {
  const start = Date.now();
  res.on('finish', () => {
    const duration = Date.now() - start;
    console.log(`${req.method} ${req.path} ${res.statusCode} ${duration}ms`);
    // AUD-05 FR-6: Record request metrics
    if (req.path !== '/metrics' && req.path !== '/health') {
      const status = res.statusCode < 400 ? 'ok' : 'error';
      sidecarRequestsTotal.labels(req.path, status).inc();
      sidecarLatencySeconds.labels(req.path).observe(duration / 1000);
    }
  });
  next();
});

// --- Routes ---

app.get('/health', (_req, res) => {
  sidecarPoolSize.set(pool.size());
  res.json({
    ok: true,
    poolSize: pool.size(),
    uptime: process.uptime(),
  });
});

// AUD-05 FR-6: Prometheus metrics endpoint
app.get('/metrics', async (_req, res) => {
  sidecarPoolSize.set(pool.size());
  res.set('Content-Type', register.contentType);
  res.end(await register.metrics());
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
app.post('/orders/open', handleOpenOrders);
app.post('/orders/cancel-all', handleCancelAllOrders);
app.post('/trades', handleTrades);

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
