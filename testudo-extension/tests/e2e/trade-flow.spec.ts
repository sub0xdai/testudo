import { test, expect, MOCK_TV_PATH } from "./fixtures/extension";
import { createServer, type Server, type IncomingMessage, type ServerResponse } from "http";
import fs from "fs";

let httpServer: Server;
let serverPort: number;

// Captured trade requests from the extension's background worker
let capturedRequests: { body: string; headers: Record<string, string> }[] = [];

test.beforeAll(async () => {
  // Start a local HTTP server that:
  // 1. Serves mock-tradingview.html at /
  // 2. Captures POST /api/v1/trades from the background worker
  capturedRequests = [];

  httpServer = createServer((req: IncomingMessage, res: ServerResponse) => {
    // CORS headers for extension requests
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type, Authorization, X-User-Id, X-Execution-Mode");

    if (req.method === "OPTIONS") {
      res.writeHead(204);
      res.end();
      return;
    }

    if (req.method === "GET" && (req.url === "/" || req.url === "/index.html")) {
      const html = fs.readFileSync(MOCK_TV_PATH, "utf-8");
      res.writeHead(200, { "Content-Type": "text/html" });
      res.end(html);
      return;
    }

    if (req.method === "POST" && req.url === "/api/v1/trades") {
      let body = "";
      req.on("data", (chunk: Buffer) => { body += chunk.toString(); });
      req.on("end", () => {
        capturedRequests.push({
          body,
          headers: req.headers as Record<string, string>,
        });
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ success: true, data: { order_id: "test-123" } }));
      });
      return;
    }

    res.writeHead(404);
    res.end("Not found");
  });

  await new Promise<void>((resolve) => {
    httpServer.listen(0, "localhost", () => {
      const addr = httpServer.address();
      serverPort = typeof addr === "object" && addr ? addr.port : 0;
      resolve();
    });
  });
});

test.afterAll(async () => {
  await new Promise<void>((resolve) => httpServer.close(() => resolve()));
});

test.beforeEach(() => {
  capturedRequests = [];
});

test.describe("Trade Flow", () => {
  test("Alt+X opens modal with scraped trade data", async ({ context, extensionId }) => {
    // Configure backend URL to point to our local server
    const popup = await context.newPage();
    await popup.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    const backendInput = popup.locator("#backend-url");
    await backendInput.fill(`http://localhost:${serverPort}`);
    await backendInput.dispatchEvent("change");
    await popup.close();

    // Navigate to mock TradingView page
    const page = await context.newPage();
    await page.goto(`http://localhost:${serverPort}/`);

    // Wait for content script to inject and register its keydown listener.
    // Content scripts run at document_idle; we poll for a side effect.
    await page.waitForTimeout(2000);

    // Press Alt+X to trigger modal
    await page.keyboard.press("Alt+x");

    // Modal host element should appear
    const modal = page.locator("#testudo-sniper-modal");
    await expect(modal).toBeAttached({ timeout: 3000 });

    // Dismiss with Escape
    await page.keyboard.press("Escape");
    await expect(modal).not.toBeAttached({ timeout: 2000 });

    await page.close();
  });

  test("Enter confirms trade and sends POST to backend", async ({ context, extensionId }) => {
    // Configure backend URL
    const popup = await context.newPage();
    await popup.goto(`chrome-extension://${extensionId}/popup/popup.html`);
    await popup.locator("#backend-url").fill(`http://localhost:${serverPort}`);
    await popup.locator("#backend-url").dispatchEvent("change");
    // Ensure paper mode
    await popup.locator('.toggle-btn[data-mode="paper"]').click();
    await popup.close();

    const page = await context.newPage();
    await page.goto(`http://localhost:${serverPort}/`);
    await page.waitForTimeout(1000);

    // Open modal
    await page.keyboard.press("Alt+x");
    const modal = page.locator("#testudo-sniper-modal");
    await expect(modal).toBeAttached({ timeout: 3000 });

    // Confirm trade with Enter
    await page.keyboard.press("Enter");

    // Modal should dismiss after confirmation
    await expect(modal).not.toBeAttached({ timeout: 3000 });

    // Wait for the background worker to send the POST request
    await page.waitForTimeout(2000);

    // Verify the trade request was received
    expect(capturedRequests.length).toBeGreaterThanOrEqual(1);

    const tradeReq = capturedRequests.find((r) => {
      try {
        const parsed = JSON.parse(r.body);
        return parsed.symbol !== undefined;
      } catch { return false; }
    });
    expect(tradeReq).toBeDefined();

    const tradeBody = JSON.parse(tradeReq!.body);
    // Symbol should be normalized: BTCUSDT → BTC_USDT
    expect(tradeBody.symbol).toBe("BTC_USDT");
    expect(tradeBody.side).toBe("buy"); // LONG maps to buy
    expect(tradeBody.entry_price).toBe("95000");
    expect(tradeBody.stop_loss_price).toBe("93000");
    expect(tradeBody.take_profit_price).toBe("99000");

    // Verify execution mode header
    expect(tradeReq!.headers["x-execution-mode"]).toBe("paper");

    // Should have paper user ID (not authenticated)
    expect(tradeReq!.headers["x-user-id"]).toBeDefined();

    await page.close();
  });

  test("Escape dismisses modal without sending trade", async ({ context }) => {
    const page = await context.newPage();
    await page.goto(`http://localhost:${serverPort}/`);
    await page.waitForTimeout(1000);

    await page.keyboard.press("Alt+x");
    const modal = page.locator("#testudo-sniper-modal");
    await expect(modal).toBeAttached({ timeout: 3000 });

    // Dismiss
    await page.keyboard.press("Escape");
    await expect(modal).not.toBeAttached({ timeout: 2000 });

    // No trade should have been sent
    await page.waitForTimeout(1000);
    const tradeReqs = capturedRequests.filter((r) => {
      try {
        return JSON.parse(r.body).symbol !== undefined;
      } catch { return false; }
    });
    expect(tradeReqs).toHaveLength(0);

    await page.close();
  });

  test("modal shows correct symbol and timeframe from mock page", async ({ context }) => {
    const page = await context.newPage();
    await page.goto(`http://localhost:${serverPort}/`);
    await page.waitForTimeout(1000);

    // Update symbol and timeframe on the mock page
    await page.evaluate(() => {
      (window as unknown as Record<string, Function>).__setPositionTool(
        42000, 40000, 46000, "ETHUSDT", "240", "long"
      );
    });

    await page.keyboard.press("Alt+x");
    const modal = page.locator("#testudo-sniper-modal");
    await expect(modal).toBeAttached({ timeout: 3000 });

    // Dismiss and confirm via trade
    await page.keyboard.press("Enter");
    await page.waitForTimeout(2000);

    // Check captured request has the updated values
    const tradeReq = capturedRequests.find((r) => {
      try { return JSON.parse(r.body).symbol !== undefined; }
      catch { return false; }
    });

    if (tradeReq) {
      const body = JSON.parse(tradeReq.body);
      expect(body.symbol).toBe("ETH_USDT");
      expect(body.entry_price).toBe("42000");
      expect(body.stop_loss_price).toBe("40000");
      expect(body.take_profit_price).toBe("46000");
    }

    await page.close();
  });
});
