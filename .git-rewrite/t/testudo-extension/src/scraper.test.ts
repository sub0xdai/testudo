import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { scrapeSymbol, scrapeTradeSetup, getChartApiHealth } from "./scraper";
import type { TradeSetup } from "./scraper";
import { normalizeSymbol, calculateQuantity } from "./utils";

// --- scrapeSymbol (DOM-based extraction) ---

describe("scrapeSymbol", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("extracts symbol from #header-toolbar-symbol-search", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BTCUSDT</div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  it("strips exchange prefix (BINANCE:BTCUSDT)", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BINANCE:BTCUSDT</div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  it("strips BYBIT exchange prefix", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BYBIT:ETHUSDT</div>`;
    expect(scrapeSymbol()).toBe("ETHUSDT");
  });

  it("strips .P perpetual suffix", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BTCUSDT.P</div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  it("strips PERP suffix", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BTCUSDTPERP</div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  it("strips exchange prefix and perpetual suffix together", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BINANCE:SOLUSDT.P</div>`;
    expect(scrapeSymbol()).toBe("SOLUSDT");
  });

  it("strips whitespace from symbol text", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">  BTC USDT  </div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  it("strips trailing dots", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BTCUSDT..</div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  // CEX-08: Bybit embedded TradingView widget symbol formats
  it("strips .Bybit exchange suffix", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BTCUSDT.Bybit</div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  it("strips leading dot + market prefix (.MBTCUSDT.Bybit)", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">.MBTCUSDT.Bybit</div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  it("strips .Binance exchange suffix", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">ETHUSDT.Binance</div>`;
    expect(scrapeSymbol()).toBe("ETHUSDT");
  });

  it("strips .OKX exchange suffix (case insensitive)", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">SOLUSDT.okx</div>`;
    expect(scrapeSymbol()).toBe("SOLUSDT");
  });

  it("strips leading dot without market prefix (.BTCUSDT)", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">.BTCUSDT</div>`;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });

  it("falls back to legend-source-item selector", () => {
    document.body.innerHTML = `
      <div data-name="legend-source-item">
        <span class="title-something">ETHUSDT</span>
      </div>
    `;
    expect(scrapeSymbol()).toBe("ETHUSDT");
  });

  it("falls back to symbolTitle selector", () => {
    document.body.innerHTML = `<div class="symbolTitle-abc">SOLUSDT</div>`;
    expect(scrapeSymbol()).toBe("SOLUSDT");
  });

  it("falls back to paneTitle selector", () => {
    document.body.innerHTML = `<div class="paneTitle-xyz">AVAXUSDT</div>`;
    expect(scrapeSymbol()).toBe("AVAXUSDT");
  });

  it("returns null when no matching elements exist", () => {
    document.body.innerHTML = `<div class="unrelated">nothing here</div>`;
    expect(scrapeSymbol()).toBeNull();
  });

  it("returns null for empty text content", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search"></div>`;
    expect(scrapeSymbol()).toBeNull();
  });

  it("returns null for too-short symbol (< 3 chars)", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BT</div>`;
    expect(scrapeSymbol()).toBeNull();
  });

  it("returns null for too-long symbol (> 20 chars)", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">${"A".repeat(21)}</div>`;
    expect(scrapeSymbol()).toBeNull();
  });

  it("accepts exactly 3-char symbol", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BTC</div>`;
    expect(scrapeSymbol()).toBe("BTC");
  });

  it("accepts exactly 20-char symbol", () => {
    const sym = "A".repeat(20);
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">${sym}</div>`;
    expect(scrapeSymbol()).toBe(sym);
  });

  it("prefers first matching selector", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <div class="symbolTitle-abc">ETHUSDT</div>
    `;
    expect(scrapeSymbol()).toBe("BTCUSDT");
  });
});

// --- getChartApiHealth ---

describe("getChartApiHealth", () => {
  afterEach(() => {
    delete (window as any).TradingViewApi;
    delete (window as any).ChartApiInstance;
  });

  it("returns all false when no API is available", () => {
    const health = getChartApiHealth();
    expect(health).toEqual({
      available: false,
      hasActiveChart: false,
      hasGetAllShapes: false,
      hasGetShapeById: false,
    });
  });

  it("detects TradingViewApi with full chart capabilities", () => {
    (window as any).TradingViewApi = {
      activeChart: () => ({
        getAllShapes: () => [],
        getShapeById: () => null,
      }),
    };
    const health = getChartApiHealth();
    expect(health).toEqual({
      available: true,
      hasActiveChart: true,
      hasGetAllShapes: true,
      hasGetShapeById: true,
    });
  });

  it("detects ChartApiInstance as fallback", () => {
    (window as any).ChartApiInstance = {
      activeChart: () => ({
        getAllShapes: () => [],
        getShapeById: () => null,
      }),
    };
    const health = getChartApiHealth();
    expect(health.available).toBe(true);
    expect(health.hasActiveChart).toBe(true);
  });

  it("handles widget without activeChart function", () => {
    (window as any).TradingViewApi = { version: "1.0" };
    const health = getChartApiHealth();
    expect(health).toEqual({
      available: true,
      hasActiveChart: false,
      hasGetAllShapes: false,
      hasGetShapeById: false,
    });
  });

  it("handles activeChart that returns object without shape methods", () => {
    (window as any).TradingViewApi = {
      activeChart: () => ({ someOtherMethod: () => {} }),
    };
    const health = getChartApiHealth();
    expect(health.available).toBe(true);
    expect(health.hasActiveChart).toBe(true);
    expect(health.hasGetAllShapes).toBe(false);
    expect(health.hasGetShapeById).toBe(false);
  });

  it("handles activeChart that throws", () => {
    (window as any).TradingViewApi = {
      activeChart: () => {
        throw new Error("Chart not ready");
      },
    };
    const health = getChartApiHealth();
    expect(health.available).toBe(true);
    expect(health.hasActiveChart).toBe(true);
    expect(health.hasGetAllShapes).toBe(false);
    expect(health.hasGetShapeById).toBe(false);
  });
});

// --- scrapeTradeSetup (integration with DOM strategies) ---

describe("scrapeTradeSetup", () => {
  beforeEach(() => {
    // Mock browser.storage.local for telemetry recording
    (globalThis as any).chrome = {
      storage: {
        local: {
          get: vi.fn().mockResolvedValue({}),
          set: vi.fn().mockResolvedValue(undefined),
        },
      },
    };
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete (globalThis as any).chrome;
    delete (globalThis as any).browser;
    delete (window as any).TradingViewApi;
    delete (window as any).ChartApiInstance;
  });

  it("returns null when no position tool data is found", () => {
    document.body.innerHTML = `<div id="header-toolbar-symbol-search">BTCUSDT</div>`;
    expect(scrapeTradeSetup()).toBeNull();
  });

  it("returns null when no symbol can be extracted", () => {
    // Set up a data-name dialog without a symbol element
    document.body.innerHTML = `
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    expect(scrapeTradeSetup()).toBeNull();
  });

  it("extracts LONG trade setup from data-name strategy", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.symbol).toBe("BTCUSDT");
    expect(result!.side).toBe("LONG");
    expect(result!.entry).toBe(65000);
    expect(result!.target).toBe(70000);
    expect(result!.stop).toBe(63000);
  });

  it("extracts SHORT trade setup from data-name strategy", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">ETHUSDT</div>
      <input data-name="Risk/RewardshortEntryPrice" value="3500" />
      <input data-name="Risk/RewardshortProfitLevelPrice" value="3200" />
      <input data-name="Risk/RewardshortStopLevelPrice" value="3700" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.symbol).toBe("ETHUSDT");
    expect(result!.side).toBe("SHORT");
    expect(result!.entry).toBe(3500);
    expect(result!.target).toBe(3200);
    expect(result!.stop).toBe(3700);
  });

  it("handles formatted prices with commas (US locale)", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="65,432.10" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70,000.00" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63,000.00" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.entry).toBe(65432.10);
    expect(result!.target).toBe(70000);
    expect(result!.stop).toBe(63000);
  });

  it("handles European-format prices (period thousands, comma decimal)", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCEUR</div>
      <input data-name="Risk/RewardlongEntryPrice" value="65.432,10" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70.000,00" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63.000,00" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.entry).toBe(65432.10);
    expect(result!.target).toBe(70000);
    expect(result!.stop).toBe(63000);
  });

  it("handles prices with currency symbols", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="$65,432.10" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="$70,000.00" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="$63,000.00" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.entry).toBe(65432.10);
  });

  it("handles clean number prices (no separators)", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="65432.10" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.entry).toBe(65432.10);
    expect(result!.target).toBe(70000);
  });

  it("returns null for empty price inputs", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="" />
    `;
    expect(scrapeTradeSetup()).toBeNull();
  });

  it("returns timeframe as 'unknown' when no timeframe element exists", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.timeframe).toBe("unknown");
  });

  it("extracts and normalizes timeframe from header toolbar", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <div id="header-toolbar-intervals">
        <button data-value="240" aria-checked="true">4h</button>
      </div>
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.timeframe).toBe("4h");
  });

  it("normalizes 1-minute timeframe", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <div id="header-toolbar-intervals">
        <button data-value="1" aria-checked="true">1</button>
      </div>
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.timeframe).toBe("1m");
  });

  it("normalizes daily timeframe", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <div id="header-toolbar-intervals">
        <button data-value="1D" aria-checked="true">D</button>
      </div>
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();
    expect(result!.timeframe).toBe("1D");
  });

  it("accepts strategiesOnly parameter to restrict strategies", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    // Strategy 0 is the data-name strategy, so this should find data
    const result = scrapeTradeSetup([0]);
    expect(result).not.toBeNull();
    expect(result!.entry).toBe(65000);
  });

  it("returns null when strategiesOnly excludes the available strategy", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    // Strategies 2-5 won't find data-name inputs
    const result = scrapeTradeSetup([2, 3, 4, 5]);
    expect(result).toBeNull();
  });

  it("extracts from properties dialog (strategy 1) by role='dialog'", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <div role="dialog">
        Long Position tool
        <input data-name="SomeEntryPrice" value="65000" />
        <input data-name="SomeProfitLevelPrice" value="70000" />
        <input data-name="SomeStopLevelPrice" value="63000" />
      </div>
    `;
    // Strategy 1 looks for dialog by role and scans inputs with "Price" in data-name
    const result = scrapeTradeSetup([1]);
    expect(result).not.toBeNull();
    expect(result!.side).toBe("LONG");
    expect(result!.entry).toBe(65000);
    expect(result!.target).toBe(70000);
    expect(result!.stop).toBe(63000);
  });

  it("extracts SHORT from properties dialog", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <div role="dialog">
        Short Position text
        <input data-name="EntryPrice" value="65000" />
        <input data-name="ProfitLevelPrice" value="60000" />
        <input data-name="StopLevelPrice" value="67000" />
      </div>
    `;
    const result = scrapeTradeSetup([1]);
    expect(result).not.toBeNull();
    expect(result!.side).toBe("SHORT");
  });

  it("returns TradeSetup with correct shape", () => {
    document.body.innerHTML = `
      <div id="header-toolbar-symbol-search">BTCUSDT</div>
      <input data-name="Risk/RewardlongEntryPrice" value="65000" />
      <input data-name="Risk/RewardlongProfitLevelPrice" value="70000" />
      <input data-name="Risk/RewardlongStopLevelPrice" value="63000" />
    `;
    const result = scrapeTradeSetup();
    expect(result).not.toBeNull();

    // Verify the shape matches TradeSetup interface
    const keys = Object.keys(result!).sort();
    expect(keys).toEqual(["entry", "side", "stop", "symbol", "target", "timeframe"]);
    expect(typeof result!.symbol).toBe("string");
    expect(typeof result!.side).toBe("string");
    expect(typeof result!.entry).toBe("number");
    expect(typeof result!.stop).toBe("number");
    expect(typeof result!.target).toBe("number");
    expect(typeof result!.timeframe).toBe("string");
  });
});

// --- normalizeSymbol (used by scraper pipeline for backend submission) ---

describe("normalizeSymbol (scraper dependency)", () => {
  it("converts BTCUSDT to BTC_USDT", () => {
    expect(normalizeSymbol("BTCUSDT")).toBe("BTC_USDT");
  });

  it("converts ETHUSDC to ETH_USDC", () => {
    expect(normalizeSymbol("ETHUSDC")).toBe("ETH_USDC");
  });

  it("upgrades USD to USDT (BTCUSD -> BTC_USDT)", () => {
    expect(normalizeSymbol("BTCUSD")).toBe("BTC_USDT");
  });

  it("handles lowercase input", () => {
    expect(normalizeSymbol("btcusdt")).toBe("BTC_USDT");
  });

  it("returns uppercase for unknown symbols", () => {
    expect(normalizeSymbol("XYZ")).toBe("XYZ");
  });

  it("does not split a quote currency alone", () => {
    expect(normalizeSymbol("USDT")).toBe("USDT");
  });
});

// --- calculateQuantity (used by scraper pipeline for risk sizing) ---

describe("calculateQuantity (scraper dependency)", () => {
  it("calculates from entry/stop distance and risk amount", () => {
    // entry=65000, stop=63000, risk=100 -> 100/2000 = 0.05
    expect(calculateQuantity(65000, 63000, 100)).toBe(0.05);
  });

  it("handles SHORT direction (stop > entry)", () => {
    // entry=65000, stop=67000, risk=100 -> 100/2000 = 0.05
    expect(calculateQuantity(65000, 67000, 100)).toBe(0.05);
  });

  it("defaults risk to 100 when omitted", () => {
    // entry=50000, stop=49000 -> 100/1000 = 0.1
    expect(calculateQuantity(50000, 49000)).toBe(0.1);
  });

  it("returns 0.001 when entry equals stop (zero distance)", () => {
    expect(calculateQuantity(65000, 65000, 100)).toBe(0.001);
  });

  it("rounds to 8 decimal places", () => {
    // entry=100, stop=97, risk=100 -> 100/3 = 33.33333333...
    const result = calculateQuantity(100, 97, 100);
    const decimals = result.toString().split(".")[1]?.length ?? 0;
    expect(decimals).toBeLessThanOrEqual(8);
  });

  it("handles very small price differences", () => {
    // entry=0.001, stop=0.0009, risk=100 -> 100/0.0001 = 1000000
    expect(calculateQuantity(0.001, 0.0009, 100)).toBe(1000000);
  });

  it("handles very large prices", () => {
    // entry=100000, stop=99000, risk=100 -> 100/1000 = 0.1
    expect(calculateQuantity(100000, 99000, 100)).toBe(0.1);
  });
});
