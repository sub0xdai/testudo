import { describe, it, expect } from "bun:test";
import { toExchangeSymbol, toBackendSymbol } from "../src/symbols";
import type { Market } from "safe-cex/dist/types";

function mockMarket(symbol: string, base: string, quote: string): Market {
  return {
    id: symbol.toLowerCase(),
    symbol,
    base,
    quote,
    active: true,
    precision: { amount: 8, price: 2 },
    limits: {
      amount: { min: 0.001, max: 1000 },
      leverage: { min: 1, max: 125 },
    },
  };
}

describe("toExchangeSymbol", () => {
  it("converts BTC_USDT to BTCUSDT", () => {
    expect(toExchangeSymbol("BTC_USDT")).toBe("BTCUSDT");
  });

  it("converts ETH_USDT to ETHUSDT", () => {
    expect(toExchangeSymbol("ETH_USDT")).toBe("ETHUSDT");
  });

  it("converts SOL_USDT to SOLUSDT", () => {
    expect(toExchangeSymbol("SOL_USDT")).toBe("SOLUSDT");
  });

  it("converts USDC pair", () => {
    expect(toExchangeSymbol("BTC_USDC")).toBe("BTCUSDC");
  });

  it("passes through already-formatted symbols", () => {
    expect(toExchangeSymbol("BTCUSDT")).toBe("BTCUSDT");
  });

  it("handles exotic symbols", () => {
    expect(toExchangeSymbol("1000PEPE_USDT")).toBe("1000PEPEUSDT");
  });
});

describe("toBackendSymbol", () => {
  describe("with market data", () => {
    const markets = [
      mockMarket("BTCUSDT", "BTC", "USDT"),
      mockMarket("ETHUSDT", "ETH", "USDT"),
      mockMarket("SOLUSDT", "SOL", "USDT"),
      mockMarket("1000PEPEUSDT", "1000PEPE", "USDT"),
      mockMarket("BTCUSDC", "BTC", "USDC"),
    ];

    it("converts BTCUSDT to BTC_USDT", () => {
      expect(toBackendSymbol("BTCUSDT", markets)).toBe("BTC_USDT");
    });

    it("converts ETHUSDT to ETH_USDT", () => {
      expect(toBackendSymbol("ETHUSDT", markets)).toBe("ETH_USDT");
    });

    it("converts SOLUSDT to SOL_USDT", () => {
      expect(toBackendSymbol("SOLUSDT", markets)).toBe("SOL_USDT");
    });

    it("handles edge case 1000PEPEUSDT", () => {
      expect(toBackendSymbol("1000PEPEUSDT", markets)).toBe("1000PEPE_USDT");
    });

    it("handles USDC pairs", () => {
      expect(toBackendSymbol("BTCUSDC", markets)).toBe("BTC_USDC");
    });
  });

  describe("without market data (fallback)", () => {
    it("converts BTCUSDT to BTC_USDT", () => {
      expect(toBackendSymbol("BTCUSDT")).toBe("BTC_USDT");
    });

    it("converts ETHUSDT to ETH_USDT", () => {
      expect(toBackendSymbol("ETHUSDT")).toBe("ETH_USDT");
    });

    it("converts SOLUSDT to SOL_USDT", () => {
      expect(toBackendSymbol("SOLUSDT")).toBe("SOL_USDT");
    });

    it("converts 1000PEPEUSDT to 1000PEPE_USDT", () => {
      expect(toBackendSymbol("1000PEPEUSDT")).toBe("1000PEPE_USDT");
    });

    it("converts USDC pairs", () => {
      expect(toBackendSymbol("BTCUSDC")).toBe("BTC_USDC");
    });

    it("passes through unknown symbols", () => {
      expect(toBackendSymbol("UNKNOWN")).toBe("UNKNOWN");
    });
  });

  describe("roundtrip", () => {
    it("BTC_USDT roundtrips", () => {
      const exchange = toExchangeSymbol("BTC_USDT");
      expect(toBackendSymbol(exchange)).toBe("BTC_USDT");
    });

    it("ETH_USDT roundtrips", () => {
      const exchange = toExchangeSymbol("ETH_USDT");
      expect(toBackendSymbol(exchange)).toBe("ETH_USDT");
    });

    it("1000PEPE_USDT roundtrips", () => {
      const exchange = toExchangeSymbol("1000PEPE_USDT");
      expect(toBackendSymbol(exchange)).toBe("1000PEPE_USDT");
    });
  });
});
