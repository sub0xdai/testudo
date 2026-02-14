import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";

// Mock WebSocket globally before anything imports
class MockWebSocket {
  static instances: MockWebSocket[] = [];
  url: string;
  onopen: ((ev: Event) => void) | null = null;
  onclose: ((ev: CloseEvent) => void) | null = null;
  onerror: ((ev: Event) => void) | null = null;
  onmessage: ((ev: MessageEvent) => void) | null = null;
  readyState = 0;
  send = vi.fn();
  close = vi.fn();

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  simulateOpen(): void {
    this.readyState = 1;
    this.onopen?.(new Event("open"));
  }

  simulateMessage(data: unknown): void {
    this.onmessage?.(new MessageEvent("message", { data: JSON.stringify(data) }));
  }

  simulateClose(): void {
    this.readyState = 3;
    this.onclose?.(new CloseEvent("close"));
  }
}

vi.stubGlobal("WebSocket", MockWebSocket);

// Mock fetch
const mockFetch = vi.fn();
vi.stubGlobal("fetch", mockFetch);

// Shared mock storage
let mockStorage: Record<string, unknown> = {};

// Mock webextension-polyfill with a factory that uses shared state
vi.mock("webextension-polyfill", () => {
  const storage = {
    local: {
      get: vi.fn(async (keys: string[]) => {
        const result: Record<string, unknown> = {};
        for (const key of keys) {
          if (key in mockStorage) result[key] = mockStorage[key];
        }
        return result;
      }),
      set: vi.fn(async (items: Record<string, unknown>) => {
        Object.assign(mockStorage, items);
      }),
      remove: vi.fn(async (keys: string[]) => {
        for (const key of keys) delete mockStorage[key];
      }),
    },
    onChanged: { addListener: vi.fn() },
  };

  return {
    default: {
      storage,
      runtime: {
        sendMessage: vi.fn(async () => undefined),
        onMessage: { addListener: vi.fn() },
        onInstalled: { addListener: vi.fn() },
      },
      tabs: {
        query: vi.fn(async () => []),
        sendMessage: vi.fn(async () => undefined),
      },
    },
  };
});

describe("background message router", () => {
  let messageHandler: (message: unknown) => unknown;
  let browser: Record<string, unknown>;

  beforeEach(async () => {
    vi.resetModules();
    MockWebSocket.instances = [];
    mockFetch.mockReset();
    mockStorage = {};

    // Import background to trigger side effects
    await import("./background");

    // Get the mock module instance that background.ts used
    const polyfill = await import("webextension-polyfill");
    browser = polyfill.default as unknown as Record<string, unknown>;

    // Capture the message handler registered by background.ts
    const runtime = browser.runtime as { onMessage: { addListener: Mock } };
    const calls = runtime.onMessage.addListener.mock.calls;
    expect(calls.length).toBeGreaterThan(0);
    messageHandler = calls[0][0];
  });

  // Helper to access browser sub-objects
  function storage() {
    return (browser.storage as { local: { get: Mock; set: Mock; remove: Mock } }).local;
  }
  function runtime() {
    return browser.runtime as { sendMessage: Mock };
  }
  function tabs() {
    return browser.tabs as { query: Mock; sendMessage: Mock };
  }

  // --- GET_SETTINGS ---

  describe("GET_SETTINGS", () => {
    it("returns default settings when storage is empty", async () => {
      const result = await messageHandler({ type: "GET_SETTINGS" });
      expect(result).toEqual({
        backendUrl: "http://localhost:8080",
        wsUrl: "ws://localhost:4000",
        executionMode: "paper",
      });
    });

    it("returns stored settings when available", async () => {
      mockStorage.backendUrl = "http://myserver:9090";
      mockStorage.wsUrl = "ws://myserver:5000";
      mockStorage.executionMode = "live";

      const result = await messageHandler({ type: "GET_SETTINGS" });
      expect(result).toEqual({
        backendUrl: "http://myserver:9090",
        wsUrl: "ws://myserver:5000",
        executionMode: "live",
      });
    });
  });

  // --- AUTH_STATUS ---

  describe("AUTH_STATUS", () => {
    it("returns unauthenticated when no tokens stored", async () => {
      const result = await messageHandler({ type: "AUTH_STATUS" });
      expect(result).toEqual({ authenticated: false });
    });

    it("returns authenticated with email when valid JWT stored", async () => {
      const payload = btoa(JSON.stringify({ email: "test@example.com" }));
      mockStorage.accessToken = `header.${payload}.signature`;
      mockStorage.refreshToken = "refresh-token";
      mockStorage.tokenExpiry = Math.floor(Date.now() / 1000) + 3600;

      const result = await messageHandler({ type: "AUTH_STATUS" });
      expect(result).toEqual({ authenticated: true, email: "test@example.com" });
    });

    it("returns unauthenticated when token is expired", async () => {
      const payload = btoa(JSON.stringify({ email: "test@example.com" }));
      mockStorage.accessToken = `header.${payload}.signature`;
      mockStorage.refreshToken = "refresh-token";
      mockStorage.tokenExpiry = Math.floor(Date.now() / 1000) - 100;

      const result = await messageHandler({ type: "AUTH_STATUS" });
      expect(result).toEqual({ authenticated: false });
    });
  });

  // --- LOGOUT ---

  describe("LOGOUT", () => {
    it("clears stored tokens", async () => {
      mockStorage.accessToken = "token";
      mockStorage.refreshToken = "refresh";
      mockStorage.tokenExpiry = 9999999999;

      const result = await messageHandler({ type: "LOGOUT" });
      expect(result).toEqual({ success: true });
      expect(storage().remove).toHaveBeenCalledWith([
        "accessToken", "refreshToken", "tokenExpiry",
      ]);
    });
  });

  // --- LOGIN ---

  describe("LOGIN", () => {
    it("stores tokens on successful login", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          user: { id: "user-1", email: "test@example.com" },
          tokens: { access_token: "access-123", refresh_token: "refresh-456", expires_in: 3600 },
        }),
      });

      const result = await messageHandler({
        type: "LOGIN", email: "test@example.com", password: "password123",
      });

      expect(result).toEqual({ success: true });
      expect(mockFetch).toHaveBeenCalledWith(
        "http://localhost:8080/api/v1/auth/login",
        expect.objectContaining({ method: "POST" }),
      );
      expect(mockStorage.accessToken).toBe("access-123");
      expect(mockStorage.refreshToken).toBe("refresh-456");
    });

    it("returns error on failed login", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: false, status: 401,
        json: async () => ({ message: "Invalid credentials" }),
      });

      const result = await messageHandler({
        type: "LOGIN", email: "bad@example.com", password: "wrong",
      });
      expect(result).toEqual({ success: false, error: "Invalid credentials" });
    });

    it("returns error on network failure", async () => {
      mockFetch.mockRejectedValueOnce(new Error("Connection refused"));

      const result = await messageHandler({
        type: "LOGIN", email: "test@example.com", password: "password",
      });
      expect(result).toEqual({ success: false, error: "Connection refused" });
    });
  });

  // --- EXECUTE_TRADE ---

  describe("EXECUTE_TRADE", () => {
    it("sends correctly formatted trade request", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({ success: true, data: { id: "order-1" } }),
      });

      const result = await messageHandler({
        type: "EXECUTE_TRADE",
        payload: {
          symbol: "BTCUSDT", side: "LONG",
          entry: 50000, stop: 49000, target: 52000, timeframe: "15m",
          management: {
            risk_percent: 1.0,
            break_even_at: 50,
            trailing_stop: { enabled: false, distance_percent: 25 },
            partial_tp: { enabled: false, close_percent: 50 },
          },
        },
      });

      expect(result).toEqual({ success: true, data: { id: "order-1" } });
      const [url, options] = mockFetch.mock.calls[0];
      expect(url).toBe("http://localhost:8080/api/v1/trades");
      const body = JSON.parse(options.body);
      expect(body.symbol).toBe("BTC_USDT");
      expect(body.side).toBe("buy");
      expect(body.entry_price).toBe("50000");
      expect(body.stop_loss_price).toBe("49000");
      expect(body.take_profit_price).toBe("52000");
      expect(body.quantity).toBeUndefined();
      expect(body.management).toEqual({
        risk_percent: 1.0,
        break_even_at: 50,
        trailing_stop: { enabled: false, distance_percent: 25 },
        partial_tp: { enabled: false, close_percent: 50 },
      });
    });

    it("sends X-User-Id header for paper trading (no JWT)", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true, json: async () => ({ success: true }),
      });

      await messageHandler({
        type: "EXECUTE_TRADE",
        payload: {
          symbol: "ETHUSDT", side: "SHORT",
          entry: 3000, stop: 3100, target: 2800, timeframe: "1h",
          management: {
            risk_percent: 1.0, break_even_at: 50,
            trailing_stop: { enabled: false, distance_percent: 25 },
            partial_tp: { enabled: false, close_percent: 50 },
          },
        },
      });

      const headers = mockFetch.mock.calls[0][1].headers;
      expect(headers["X-User-Id"]).toBe("00000000-0000-0000-0000-000000000001");
      expect(headers["X-Execution-Mode"]).toBe("paper");
      expect(headers["Authorization"]).toBeUndefined();
    });

    it("maps SHORT to sell", async () => {
      mockFetch.mockResolvedValueOnce({
        ok: true, json: async () => ({ success: true }),
      });

      await messageHandler({
        type: "EXECUTE_TRADE",
        payload: {
          symbol: "SOLUSDT", side: "SHORT",
          entry: 100, stop: 110, target: 80, timeframe: "4h",
          management: {
            risk_percent: 1.0, break_even_at: 50,
            trailing_stop: { enabled: false, distance_percent: 25 },
            partial_tp: { enabled: false, close_percent: 50 },
          },
        },
      });

      const body = JSON.parse(mockFetch.mock.calls[0][1].body);
      expect(body.side).toBe("sell");
    });
  });

  // --- WS_STATUS ---

  describe("WS_STATUS", () => {
    it("returns current WS state", async () => {
      const result = await messageHandler({ type: "WS_STATUS" });
      expect(result).toHaveProperty("state");
      expect(["disconnected", "connecting", "connected"]).toContain(
        (result as { state: string }).state,
      );
    });
  });

  // --- WS_RECONNECT ---

  describe("WS_RECONNECT", () => {
    it("returns success", async () => {
      const result = await messageHandler({ type: "WS_RECONNECT" });
      expect(result).toEqual({ success: true });
    });

    it("creates a new WebSocket connection", async () => {
      const countBefore = MockWebSocket.instances.length;
      await messageHandler({ type: "WS_RECONNECT" });
      await new Promise((r) => setTimeout(r, 10));
      expect(MockWebSocket.instances.length).toBeGreaterThan(countBefore);
    });
  });

  // --- WebSocket lifecycle ---

  describe("WebSocket connection", () => {
    it("connects on module load", () => {
      expect(MockWebSocket.instances.length).toBeGreaterThan(0);
    });

    it("subscribes to order channel on open", async () => {
      const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];
      ws.simulateOpen();
      await new Promise((r) => setTimeout(r, 10));

      expect(ws.send).toHaveBeenCalled();
      const msg = JSON.parse(ws.send.mock.calls[0][0]);
      expect(msg.method).toBe("SUBSCRIBE");
      expect(msg.params[0]).toMatch(/^order\./);
    });

    it("broadcasts connected state on open", async () => {
      const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];
      ws.simulateOpen();
      await new Promise((r) => setTimeout(r, 10));

      expect(runtime().sendMessage).toHaveBeenCalledWith(
        expect.objectContaining({ type: "WS_STATE_CHANGED", state: "connected" }),
      );
    });

    it("broadcasts disconnected state on close", async () => {
      const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];
      ws.simulateOpen();
      await new Promise((r) => setTimeout(r, 10));

      runtime().sendMessage.mockClear();
      ws.simulateClose();

      expect(runtime().sendMessage).toHaveBeenCalledWith(
        expect.objectContaining({ type: "WS_STATE_CHANGED", state: "disconnected" }),
      );
    });

    it("forwards order updates to TradingView tabs", async () => {
      tabs().query.mockResolvedValueOnce([
        { id: 1, url: "https://www.tradingview.com/chart/" },
      ]);

      const ws = MockWebSocket.instances[MockWebSocket.instances.length - 1];
      ws.simulateOpen();
      await new Promise((r) => setTimeout(r, 10));

      ws.simulateMessage({
        stream: "order.00000000-0000-0000-0000-000000000001",
        data: { e: "order", s: "BTC_USDT", status: "filled" },
      });
      await new Promise((r) => setTimeout(r, 10));

      expect(tabs().query).toHaveBeenCalledWith({ url: ["*://*.tradingview.com/*", "*://*.dexscreener.com/*", "*://*.gmx.io/*", "*://*.bybit.com/*"] });
      expect(tabs().sendMessage).toHaveBeenCalledWith(1, {
        type: "WS_ORDER_UPDATE",
        data: { e: "order", s: "BTC_USDT", status: "filled" },
      });
    });
  });
});
