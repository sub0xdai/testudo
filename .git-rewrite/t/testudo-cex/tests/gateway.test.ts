import { describe, it, expect, mock, beforeEach } from "bun:test";
import { ExchangeGateway, type Credentials } from "../src/gateway";

// Mock safe-cex createExchange
const mockExchange = () => {
  const listeners = new Map<string, Function[]>();
  return {
    on: mock((event: string, cb: Function) => {
      const arr = listeners.get(event) || [];
      arr.push(cb);
      listeners.set(event, arr);
    }),
    start: mock(() => Promise.resolve()),
    dispose: mock(() => {}),
    store: {},
    isDisposed: false,
    _listeners: listeners,
    _emit(event: string, ...args: any[]) {
      (listeners.get(event) || []).forEach((cb) => cb(...args));
    },
  };
};

// We need to mock the createExchange import
// Since safe-cex is a complex library with WebSocket connections,
// we test the gateway logic by mocking at the module level
const mockExchangeInstance = mockExchange();
let createExchangeCallCount = 0;

mock.module("safe-cex", () => ({
  createExchange: (..._args: any[]) => {
    createExchangeCallCount++;
    return mockExchangeInstance;
  },
}));

const testCredentials: Credentials = {
  key: "test-api-key",
  secret: "test-api-secret",
};

describe("ExchangeGateway", () => {
  let gateway: ExchangeGateway;

  beforeEach(() => {
    gateway = new ExchangeGateway();
    createExchangeCallCount = 0;
    mockExchangeInstance.start.mockClear();
    mockExchangeInstance.start.mockImplementation(() => Promise.resolve());
    mockExchangeInstance.on.mockClear();
    mockExchangeInstance.dispose.mockClear();
    mockExchangeInstance._listeners.clear();
  });

  describe("cacheKey", () => {
    it("derives deterministic hash from exchange_id + api_key + sandbox", () => {
      const key1 = gateway.cacheKey("woo", "key1", false);
      const key2 = gateway.cacheKey("woo", "key1", false);
      expect(key1).toBe(key2);
      expect(key1).toHaveLength(16);
    });

    it("produces different keys for different inputs", () => {
      const key1 = gateway.cacheKey("woo", "key1", false);
      const key2 = gateway.cacheKey("woo", "key1", true);
      const key3 = gateway.cacheKey("binance", "key1", false);
      const key4 = gateway.cacheKey("woo", "key2", false);
      expect(new Set([key1, key2, key3, key4]).size).toBe(4);
    });
  });

  describe("getOrCreate", () => {
    it("creates a new exchange instance", async () => {
      const onFill = mock(() => {});
      const exchange = await gateway.getOrCreate(
        "woo",
        testCredentials,
        false,
        onFill
      );

      expect(exchange).toBeDefined();
      expect(createExchangeCallCount).toBe(1);
      expect(gateway.size).toBe(1);
    });

    it("returns cached instance on duplicate call", async () => {
      const onFill = mock(() => {});
      const first = await gateway.getOrCreate(
        "woo",
        testCredentials,
        false,
        onFill
      );
      const second = await gateway.getOrCreate(
        "woo",
        testCredentials,
        false,
        onFill
      );

      expect(first).toBe(second);
      expect(createExchangeCallCount).toBe(1);
      expect(gateway.size).toBe(1);
    });

    it("wires fill, error, and log event handlers", async () => {
      const onFill = mock(() => {});
      await gateway.getOrCreate("woo", testCredentials, false, onFill);

      const onCalls = mockExchangeInstance.on.mock.calls;
      const events = onCalls.map((c: any[]) => c[0]);
      expect(events).toContain("fill");
      expect(events).toContain("error");
      expect(events).toContain("log");
    });

    it("calls exchange.start() during creation", async () => {
      const onFill = mock(() => {});
      await gateway.getOrCreate("woo", testCredentials, false, onFill);

      expect(mockExchangeInstance.start).toHaveBeenCalledTimes(1);
    });

    it("does not cache instance if start() fails", async () => {
      mockExchangeInstance.start.mockImplementation(() =>
        Promise.reject(new Error("connection failed"))
      );

      const onFill = mock(() => {});
      await expect(
        gateway.getOrCreate("woo", testCredentials, false, onFill)
      ).rejects.toThrow("connection failed");

      expect(gateway.size).toBe(0);
    });
  });

  describe("dispose", () => {
    it("removes instance from cache and calls dispose", async () => {
      const onFill = mock(() => {});
      await gateway.getOrCreate("woo", testCredentials, false, onFill);
      expect(gateway.size).toBe(1);

      const key = gateway.cacheKey("woo", testCredentials.key, false);
      await gateway.dispose(key);

      expect(gateway.size).toBe(0);
      expect(mockExchangeInstance.dispose).toHaveBeenCalledTimes(1);
    });

    it("does nothing for non-existent key", async () => {
      await gateway.dispose("nonexistent");
      expect(gateway.size).toBe(0);
    });
  });

  describe("disposeAll", () => {
    it("disposes all instances", async () => {
      const onFill = mock(() => {});
      await gateway.getOrCreate("woo", testCredentials, false, onFill);
      expect(gateway.size).toBe(1);

      await gateway.disposeAll();
      expect(gateway.size).toBe(0);
    });
  });
});
