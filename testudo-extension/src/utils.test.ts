import { describe, it, expect } from "vitest";
import {
  normalizeSymbol,
  calculateQuantity,
  mapSide,
  calculateRefreshDelay,
  nextReconnectDelay,
  DEFAULT_SETTINGS,
  QUOTE_CURRENCIES,
  WS_MAX_RECONNECT_DELAY,
  WS_BASE_RECONNECT_DELAY,
} from "./utils";

// --- normalizeSymbol ---

describe("normalizeSymbol", () => {
  it("converts BTCUSDT to BTC_USDT", () => {
    expect(normalizeSymbol("BTCUSDT")).toBe("BTC_USDT");
  });

  it("converts ETHUSDC to ETH_USDC", () => {
    expect(normalizeSymbol("ETHUSDC")).toBe("ETH_USDC");
  });

  it("converts SOLUSDT to SOL_USDT", () => {
    expect(normalizeSymbol("SOLUSDT")).toBe("SOL_USDT");
  });

  it("handles lowercase input", () => {
    expect(normalizeSymbol("btcusdt")).toBe("BTC_USDT");
  });

  it("handles mixed case input", () => {
    expect(normalizeSymbol("BtcUsdt")).toBe("BTC_USDT");
  });

  it("converts BTC-quote pairs", () => {
    expect(normalizeSymbol("ETHBTC")).toBe("ETH_BTC");
  });

  it("converts BTCUSD to BTC_USDT (USD -> USDT upgrade)", () => {
    expect(normalizeSymbol("BTCUSD")).toBe("BTC_USDT");
  });

  it("converts ETHUSD to ETH_USDT (USD -> USDT upgrade)", () => {
    expect(normalizeSymbol("ETHUSD")).toBe("ETH_USDT");
  });

  it("converts SOLUSD to SOL_USDT (USD -> USDT upgrade)", () => {
    expect(normalizeSymbol("SOLUSD")).toBe("SOL_USDT");
  });

  it("converts EUR-quote pairs", () => {
    expect(normalizeSymbol("BTCEUR")).toBe("BTC_EUR");
  });

  it("converts BUSD pairs", () => {
    expect(normalizeSymbol("SOLBUSD")).toBe("SOL_BUSD");
  });

  it("converts FDUSD pairs", () => {
    expect(normalizeSymbol("BTCFDUSD")).toBe("BTC_FDUSD");
  });

  it("returns uppercase for unknown symbols", () => {
    expect(normalizeSymbol("XYZ")).toBe("XYZ");
  });

  it("does not split a quote currency alone", () => {
    // "USDT" alone should not become "_USDT"
    expect(normalizeSymbol("USDT")).toBe("USDT");
  });

  it("prefers longer quote match (USDT over USD)", () => {
    // BTCUSDT should match USDT, not USD
    expect(normalizeSymbol("BTCUSDT")).toBe("BTC_USDT");
  });
});

// --- calculateQuantity ---

describe("calculateQuantity", () => {
  it("calculates quantity from entry and stop distance", () => {
    // entry=100, stop=90, risk=100 → 100/10 = 10
    expect(calculateQuantity(100, 90, 100)).toBe(10);
  });

  it("uses default risk amount of 100", () => {
    // entry=50000, stop=49000 → 100/1000 = 0.1
    expect(calculateQuantity(50000, 49000)).toBe(0.1);
  });

  it("handles stop above entry (short position)", () => {
    // entry=100, stop=110, risk=100 → 100/10 = 10
    expect(calculateQuantity(100, 110, 100)).toBe(10);
  });

  it("returns 0.001 when stop equals entry (zero distance)", () => {
    expect(calculateQuantity(100, 100, 100)).toBe(0.001);
  });

  it("rounds to 8 decimal places", () => {
    // entry=100, stop=97, risk=100 → 100/3 = 33.33333333...
    const result = calculateQuantity(100, 97, 100);
    const decimals = result.toString().split(".")[1]?.length ?? 0;
    expect(decimals).toBeLessThanOrEqual(8);
  });

  it("handles very small stop distance", () => {
    // entry=0.001, stop=0.0009, risk=100 → 100/0.0001 = 1000000
    const result = calculateQuantity(0.001, 0.0009, 100);
    expect(result).toBe(1000000);
  });

  it("handles very large values", () => {
    // entry=100000, stop=99000, risk=100 → 100/1000 = 0.1
    expect(calculateQuantity(100000, 99000, 100)).toBe(0.1);
  });
});

// --- mapSide ---

describe("mapSide", () => {
  it('maps LONG to "buy"', () => {
    expect(mapSide("LONG")).toBe("buy");
  });

  it('maps SHORT to "sell"', () => {
    expect(mapSide("SHORT")).toBe("sell");
  });
});

// --- calculateRefreshDelay ---

describe("calculateRefreshDelay", () => {
  it("returns (expiresIn - 60) * 1000 for normal expiry", () => {
    expect(calculateRefreshDelay(3600)).toBe(3540000);
  });

  it("returns minimum 10 seconds when expiry is very short", () => {
    expect(calculateRefreshDelay(30)).toBe(10000);
  });

  it("returns minimum 10 seconds when expiry is exactly 60", () => {
    // expiresIn=60 → max(10, 0) * 1000 = 10000
    expect(calculateRefreshDelay(60)).toBe(10000);
  });

  it("returns minimum 10 seconds when expiry is less than 60", () => {
    expect(calculateRefreshDelay(5)).toBe(10000);
  });

  it("handles large expiry values", () => {
    expect(calculateRefreshDelay(86400)).toBe(86340000);
  });
});

// --- nextReconnectDelay ---

describe("nextReconnectDelay", () => {
  it("doubles the current delay", () => {
    expect(nextReconnectDelay(1000)).toBe(2000);
  });

  it("caps at WS_MAX_RECONNECT_DELAY", () => {
    expect(nextReconnectDelay(20000)).toBe(WS_MAX_RECONNECT_DELAY);
  });

  it("returns max when already at max", () => {
    expect(nextReconnectDelay(WS_MAX_RECONNECT_DELAY)).toBe(WS_MAX_RECONNECT_DELAY);
  });

  it("doubles from base delay", () => {
    expect(nextReconnectDelay(WS_BASE_RECONNECT_DELAY)).toBe(2000);
  });
});

// --- Constants ---

describe("constants", () => {
  it("has correct default settings", () => {
    expect(DEFAULT_SETTINGS.backendUrl).toBe("https://api.testudo.vip");
    expect(DEFAULT_SETTINGS.wsUrl).toBe("wss://ws.testudo.vip");
  });

  it("has quote currencies list", () => {
    expect(QUOTE_CURRENCIES).toContain("USDT");
    expect(QUOTE_CURRENCIES).toContain("USDC");
    expect(QUOTE_CURRENCIES).toContain("BTC");
    expect(QUOTE_CURRENCIES.length).toBeGreaterThan(5);
  });

  it("has reasonable WS reconnect bounds", () => {
    expect(WS_BASE_RECONNECT_DELAY).toBe(1000);
    expect(WS_MAX_RECONNECT_DELAY).toBe(30000);
    expect(WS_MAX_RECONNECT_DELAY).toBeGreaterThan(WS_BASE_RECONNECT_DELAY);
  });
});
