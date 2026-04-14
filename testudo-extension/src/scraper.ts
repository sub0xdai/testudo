// EXT-02: TradingView DOM Scraper
// Extracts trade setup data from TradingView's Long/Short Position drawing tools.
// Uses multiple selector strategies with fallbacks for resilience.

import type { ScraperHealthRecord, ChartApiHealth } from "./types";

// MV3 provides Promise-based APIs natively — no polyfill needed in content scripts
const browser = (globalThis as any).browser ?? (globalThis as any).chrome;

export interface TradeSetup {
  symbol: string;
  side: "LONG" | "SHORT";
  entry: number;
  stop: number;
  target: number;
  timeframe: string;
}

const SCRAPER_HEALTH_MAX = 20;

// --- Price Parsing ---

function parsePrice(raw: string): number | null {
  if (!raw) return null;
  // Strip currency symbols, whitespace, and thousand separators
  // Handle both comma-as-thousands (95,000.50) and period-as-thousands (95.000,50)
  let cleaned = raw.trim().replace(/[^\d.,\-]/g, "");
  if (!cleaned) return null;

  // Detect format: if last separator is comma and has <=2 digits after, it's decimal comma
  const lastComma = cleaned.lastIndexOf(",");
  const lastDot = cleaned.lastIndexOf(".");

  if (lastComma > lastDot && cleaned.length - lastComma <= 3) {
    // European format: 95.000,50 → 95000.50
    cleaned = cleaned.replace(/\./g, "").replace(",", ".");
  } else {
    // Standard format: 95,000.50 → 95000.50
    cleaned = cleaned.replace(/,/g, "");
  }

  const value = parseFloat(cleaned);
  return isNaN(value) ? null : value;
}

// --- Symbol Extraction ---

const SYMBOL_SELECTORS = [
  "#header-toolbar-symbol-search",
  '[data-name="legend-source-item"] [class*="title"]',
  '[class*="symbolTitle"]',
  '[class*="paneTitle"]',
];

// --- Hyperliquid Symbol Extraction (EXT-44) ---
// Hyperliquid uses styled-components with unstable hash classes.
// Match leaf divs by text content pattern instead of class selectors.

function scrapeHyperliquidSymbol(): string | null {
  // Strategy 1: Walk leaf divs for "BTC-USDC" pattern
  const divs = document.querySelectorAll("div");
  for (const div of divs) {
    if (div.children.length > 0) continue;
    const text = div.textContent?.trim() || "";
    if (/^[A-Z0-9]{2,10}-USDC$/.test(text)) {
      return text.replace("-", ""); // BTC-USDC → BTCUSDC
    }
  }

  // Strategy 2: document.title fallback
  const match = document.title.match(/([A-Z0-9]{2,10})-USDC/);
  if (match) return match[0].replace("-", "");

  return null;
}

// --- DexScreener Symbol Extraction (EXT-45) ---
// DexScreener uses Chakra UI with hashed CSS classes (unstable).
// It embeds TradingView's charting library directly, so legend selectors may work.
// 4 fallback strategies: legend, title parens, title slash, leaf element scan.

function scrapeDexScreenerSymbol(): string | null {
  // Strategy 1: TradingView legend (charting lib is embedded, not iframe)
  // Legend shows "TOKEN/QUOTE (description) on DEX · timeframe"
  for (const sel of [
    '[data-name="legend-source-item"] [class*="title"]',
    '[class*="legendTitle"]',
  ]) {
    const el = document.querySelector(sel);
    if (el?.textContent) {
      const match = el.textContent.trim().match(/^([A-Za-z0-9]+)\/([A-Za-z]+)/);
      if (match) return (match[1] + match[2]).toUpperCase();
    }
  }

  // Strategy 2: document.title — "tokenName (SYMBOL) Price | DEX Screener"
  const parenMatch = document.title.match(/\(([A-Z0-9]{2,10})\)/);
  if (parenMatch) return parenMatch[1] + "USD";

  // Strategy 3: slash-separated pair in title — "TOKEN / SOL"
  const slashMatch = document.title.match(/([A-Za-z0-9]+)\s*\/\s*([A-Za-z]+)/);
  if (slashMatch) return (slashMatch[1] + slashMatch[2]).toUpperCase();

  // Strategy 4: scan leaf spans/divs for "XXX / YYY" pattern
  const elements = document.querySelectorAll("span, div, a");
  for (const el of elements) {
    if (el.children.length > 0) continue;
    const text = el.textContent?.trim() || "";
    if (/^[A-Z]{2,10}\s*\/\s*[A-Z]{2,10}$/.test(text)) {
      return text.replace(/\s*\/\s*/, "");
    }
  }

  return null;
}

export function scrapeSymbol(): string | null {
  // Hyperliquid has its own symbol format — check first on that platform
  if (location.hostname.includes("hyperliquid")) {
    const hlSymbol = scrapeHyperliquidSymbol();
    if (hlSymbol) return hlSymbol;
  }

  // DexScreener uses embedded TradingView charting lib — check before generic selectors
  if (location.hostname.includes("dexscreener.com")) {
    const dsSymbol = scrapeDexScreenerSymbol();
    if (dsSymbol) return dsSymbol;
  }

  for (const selector of SYMBOL_SELECTORS) {
    // EXT-46: Check main document + same-origin iframes (chart may be in blob iframe)
    const el = queryAll(selector);
    if (!el?.textContent) continue;

    // Extract symbol text, strip exchange prefix (e.g., "BINANCE:" or "BYBIT:")
    let text = el.textContent.trim();
    // Remove exchange prefix
    if (text.includes(":")) {
      text = text.split(":").pop()!.trim();
    }
    // Remove perpetual suffix (.P, PERP)
    text = text.replace(/\.P$/, "").replace(/PERP$/, "");
    // CEX-08: Strip exchange suffix (.Bybit, .Binance, .OKX, etc.)
    // Embedded TradingView widgets use format like ".MBTCUSDT.Bybit"
    text = text.replace(/\.(Bybit|Binance|OKX|Bitget|Gate|Phemex|BloFin)$/i, "");
    // Strip TradingView continuous contract prefix ".M" (e.g., ".MBTCUSDT" → "BTCUSDT")
    text = text.replace(/^\.M(?=[A-Z]{2,})/, "");
    // Strip remaining leading dots (e.g., ".BTCUSDT" → "BTCUSDT")
    text = text.replace(/^\.+/, "");
    // Remove whitespace and trailing dots
    text = text.replace(/\s+/g, "").replace(/\.+$/, "");

    if (text.length >= 3 && text.length <= 20) {
      return text;
    }
  }
  return null;
}

// --- Timeframe Extraction ---

const TIMEFRAME_SELECTORS = [
  '#header-toolbar-intervals button[data-value][aria-checked="true"]',
  '#header-toolbar-intervals [class*="isActive"] [data-value]',
  '#header-toolbar-intervals [class*="active"]',
];

export function scrapeTimeframe(): string {
  for (const selector of TIMEFRAME_SELECTORS) {
    const el = document.querySelector(selector);
    if (!el) continue;

    const dataValue = el.getAttribute("data-value");
    if (dataValue) return normalizeTimeframe(dataValue);

    const text = el.textContent?.trim();
    if (text) return normalizeTimeframe(text);
  }
  return "unknown";
}

function normalizeTimeframe(raw: string): string {
  // TradingView data-value uses minutes as base: "1", "5", "15", "60", "240", "1D", "1W", "1M"
  const map: Record<string, string> = {
    "1": "1m", "3": "3m", "5": "5m", "15": "15m", "30": "30m",
    "45": "45m", "60": "1h", "120": "2h", "180": "3h", "240": "4h",
    "360": "6h", "480": "8h", "720": "12h",
    "1D": "1D", "D": "1D", "1W": "1W", "W": "1W", "1M": "1M", "M": "1M",
  };
  return map[raw] || raw;
}

// --- Position Tool Detection ---
// TradingView's Long/Short Position tools are canvas-rendered. Price values are
// accessible via the internal chart API (window.TradingViewApi) or the properties
// dialog (opened by double-clicking the drawing tool).

interface PositionToolData {
  entry: number;
  stop: number;
  target: number;
  side: "LONG" | "SHORT";
}

function isPositiveNumber(v: unknown): v is number {
  return typeof v === "number" && v > 0;
}

function isValidSide(v: unknown): v is "LONG" | "SHORT" {
  return v === "LONG" || v === "SHORT";
}

function validatePositionToolData(v: unknown): { success: true; data: PositionToolData } | { success: false } {
  if (!v || typeof v !== "object") return { success: false };
  const o = v as Record<string, unknown>;
  if (isPositiveNumber(o.entry) && isPositiveNumber(o.stop) && isPositiveNumber(o.target) && isValidSide(o.side)) {
    return { success: true, data: { entry: o.entry, stop: o.stop, target: o.target, side: o.side } };
  }
  return { success: false };
}

function validateTradeSetup(v: unknown): { success: true; data: TradeSetup } | { success: false; error: string } {
  if (!v || typeof v !== "object") return { success: false, error: "not an object" };
  const o = v as Record<string, unknown>;
  if (
    typeof o.symbol === "string" && o.symbol.length >= 1 &&
    isValidSide(o.side) &&
    isPositiveNumber(o.entry) && isPositiveNumber(o.stop) && isPositiveNumber(o.target) &&
    typeof o.timeframe === "string" && o.timeframe.length >= 1
  ) {
    return { success: true, data: { symbol: o.symbol, side: o.side, entry: o.entry, stop: o.stop, target: o.target, timeframe: o.timeframe } };
  }
  return { success: false, error: "invalid trade setup fields" };
}

function validateHealthHistory(v: unknown): { success: true; data: ScraperHealthRecord[] } | { success: false } {
  if (!Array.isArray(v)) return { success: false };
  const valid: ScraperHealthRecord[] = [];
  for (const item of v) {
    if (
      item && typeof item === "object" &&
      typeof item.timestamp === "number" &&
      typeof item.success === "boolean" &&
      (item.strategyUsed === null || (typeof item.strategyUsed === "number" && Number.isInteger(item.strategyUsed) && item.strategyUsed >= 0 && item.strategyUsed <= 5))
    ) {
      valid.push({ timestamp: item.timestamp, strategyUsed: item.strategyUsed, success: item.success });
    }
  }
  return { success: true, data: valid };
}

// --- Strategy 0: Zero-flash — TradingView internal chart API ---
// Reads position tool data directly from window.TradingViewApi.activeChart()
// No dialog needed, no UI flash. Uses getAllShapes() + getShapeById().

function getChartApi(): any | null {
  const w = window as any;
  const widget = w.TradingViewApi || w.ChartApiInstance;
  if (!widget || typeof widget.activeChart !== "function") return null;
  return widget.activeChart();
}

interface TickSizeResult {
  value: number;
  source: string;
}

function getTickSize(chart: any, entryPrice: number): TickSizeResult {
  // Try 1: Price formatter internals
  try {
    if (typeof chart.priceFormatter === "function") {
      const fmt = chart.priceFormatter();
      // Walk own + prototype properties for _minMove/_priceScale
      if (fmt) {
        const minMove = fmt._minMove ?? fmt.minMove;
        const priceScale = fmt._priceScale ?? fmt.priceScale;
        if (typeof minMove === "number" && typeof priceScale === "number" && priceScale > 0) {
          return { value: minMove / priceScale, source: "priceFormatter" };
        }
      }
    }
  } catch { /* continue */ }

  // Try 2: Series symbol info
  try {
    if (typeof chart.getSeries === "function") {
      const series = chart.getSeries();
      if (series) {
        const info = typeof series.symbolInfo === "function"
          ? series.symbolInfo()
          : series._symbolInfo || series.symbolInfo;
        if (info) {
          const mm = info.minmov ?? info.minmovement ?? info.min_move;
          const ps = info.pricescale ?? info.price_scale;
          if (typeof mm === "number" && typeof ps === "number" && ps > 0) {
            return { value: mm / ps, source: "symbolInfo" };
          }
        }
      }
    }
  } catch { /* continue */ }

  // Fallback: derive from entry price decimal places
  const str = entryPrice.toString();
  const dot = str.indexOf(".");
  if (dot >= 0) {
    return { value: Math.pow(10, -(str.length - dot - 1)), source: "decimalFallback" };
  }
  return { value: 0.01, source: "hardcodedFallback" };
}

function findPositionToolByChartApi(): PositionToolData | null {
  const chart = getChartApi();
  if (!chart || typeof chart.getAllShapes !== "function") return null;

  const shapes: Array<{ id: string; name: string }> = chart.getAllShapes();
  if (!Array.isArray(shapes) || shapes.length === 0) return null;

  // Find position tools — prefer last one (most recently drawn)
  const positionTool = [...shapes]
    .reverse()
    .find((s) => s.name === "long_position" || s.name === "short_position");
  if (!positionTool) return null;

  if (typeof chart.getShapeById !== "function") return null;
  const api = chart.getShapeById(positionTool.id);
  if (!api) return null;

  const side: "LONG" | "SHORT" = positionTool.name === "long_position" ? "LONG" : "SHORT";

  // Get entry price from anchor points
  if (typeof api.getPoints !== "function") return null;
  const points = api.getPoints();
  if (!Array.isArray(points) || points.length === 0) return null;

  const entry = points[0]?.price;
  if (typeof entry !== "number" || entry <= 0) return null;

  // Get stop/target levels from properties
  if (typeof api.getProperties !== "function") return null;
  const props = api.getProperties();
  if (!props || typeof props.stopLevel !== "number" || typeof props.profitLevel !== "number") return null;

  const tick = getTickSize(chart, entry);
  const stopDist = props.stopLevel * tick.value;
  const profitDist = props.profitLevel * tick.value;

  const stop = side === "LONG" ? entry - stopDist : entry + stopDist;
  const target = side === "LONG" ? entry + profitDist : entry - profitDist;

  console.log("[Testudo] Strategy:ChartApi", {
    entry, side,
    stopLevel: props.stopLevel, profitLevel: props.profitLevel,
    tickSize: tick.value, tickSource: tick.source,
    stop, target,
  });

  // Validate prices
  if (stop <= 0 || target <= 0) return null;
  if (side === "LONG" && (stop >= entry || target <= entry)) return null;
  if (side === "SHORT" && (stop <= entry || target >= entry)) return null;

  return { entry, stop, target, side };
}

// EXT-46: Get all searchable documents (main + same-origin iframes).
function getDocuments(): Document[] {
  const docs: Document[] = [document];
  const iframes = document.querySelectorAll("iframe");
  for (const iframe of iframes) {
    try {
      const doc = (iframe as HTMLIFrameElement).contentDocument;
      if (doc) docs.push(doc);
    } catch { /* cross-origin — skip */ }
  }
  return docs;
}

// EXT-46: Query across main document AND same-origin iframes (e.g. Bybit blob iframe).
// TradingView charting library on exchange sites renders inside iframes.
function queryAll(selector: string): Element | null {
  for (const doc of getDocuments()) {
    const el = doc.querySelector(selector);
    if (el) return el;
  }
  return null;
}

// Data-name patterns for the properties dialog inputs (Feb 2026)
const RISK_REWARD_INPUTS = {
  long: {
    entry: 'input[data-name="Risk/RewardlongEntryPrice"]',
    target: 'input[data-name="Risk/RewardlongProfitLevelPrice"]',
    stop: 'input[data-name="Risk/RewardlongStopLevelPrice"]',
  },
  short: {
    entry: 'input[data-name="Risk/RewardshortEntryPrice"]',
    target: 'input[data-name="Risk/RewardshortProfitLevelPrice"]',
    stop: 'input[data-name="Risk/RewardshortStopLevelPrice"]',
  },
} as const;

// Strategy 1: Read from the properties dialog using data-name selectors
// This is the primary strategy — uses stable, semantic attributes.
// Requires the properties dialog to be open (double-click the position tool).
function findPositionToolByDataName(): PositionToolData | null {
  // Try long first, then short — queries main doc + same-origin iframes (EXT-46)
  for (const [side, selectors] of Object.entries(RISK_REWARD_INPUTS)) {
    const entryEl = queryAll(selectors.entry) as HTMLInputElement | null;
    const targetEl = queryAll(selectors.target) as HTMLInputElement | null;
    const stopEl = queryAll(selectors.stop) as HTMLInputElement | null;

    if (entryEl?.value && targetEl?.value && stopEl?.value) {
      const entry = parsePrice(entryEl.value);
      const target = parsePrice(targetEl.value);
      const stop = parsePrice(stopEl.value);

      if (entry !== null && target !== null && stop !== null) {
        return { entry, stop, target, side: side === "long" ? "LONG" : "SHORT" };
      }
    }
  }

  // EXT-46: Positional fallback for embedded TradingView (DexScreener, etc.)
  // where data-name attributes are missing. The dialog has a consistent input order:
  // [title, accountSize, lotSize, risk, entry, profitTicks, profitPrice, stopTicks, stopPrice]
  for (const doc of getDocuments()) {
    const inputs = doc.querySelectorAll("input");
    // Find the title input that says "Long Position" or "Short Position"
    for (let i = 0; i < inputs.length; i++) {
      const val = (inputs[i] as HTMLInputElement).value?.trim().toLowerCase();
      if (val !== "long position" && val !== "short position") continue;

      const side: "LONG" | "SHORT" = val === "long position" ? "LONG" : "SHORT";
      // Entry is 4 inputs after title, target price is 6 after, stop price is 8 after
      const entryEl = inputs[i + 4] as HTMLInputElement | undefined;
      const targetEl = inputs[i + 6] as HTMLInputElement | undefined;
      const stopEl = inputs[i + 8] as HTMLInputElement | undefined;

      if (entryEl?.value && targetEl?.value && stopEl?.value) {
        const entry = parsePrice(entryEl.value);
        const target = parsePrice(targetEl.value);
        const stop = parsePrice(stopEl.value);

        if (entry !== null && target !== null && stop !== null &&
            entry > 0 && target > 0 && stop > 0) {
          return { entry, stop, target, side };
        }
      }
    }
  }

  return null;
}

// Strategy 2: Look for the properties dialog by role="dialog" and scan inputs
// Fallback if data-name attributes change but dialog structure remains.
function findPositionToolByPropertiesDialog(): PositionToolData | null {
  const dialog = document.querySelector(
    '[data-name="source-properties-editor"], [role="dialog"]'
  );
  if (!dialog) return null;

  const text = dialog.textContent?.toLowerCase() || "";
  // Must be a position tool dialog
  const isPositionTool = text.includes("long position") || text.includes("short position");
  if (!isPositionTool) return null;

  const side: "LONG" | "SHORT" = text.includes("long position") ? "LONG" : "SHORT";

  // Find all inputs and look for entry/profit/stop price values
  const inputs = dialog.querySelectorAll("input");
  const values: number[] = [];
  for (const input of inputs) {
    const name = input.getAttribute("data-name") || "";
    if (name.includes("Price") && !name.includes("Ticks")) {
      const parsed = parsePrice(input.value);
      if (parsed !== null) values.push(parsed);
    }
  }

  // Expect entry, profit, stop (in order from dialog)
  if (values.length >= 3) {
    return { entry: values[0], target: values[1], stop: values[2], side };
  }
  return null;
}

// Strategy 3: Legacy — look for labeled inputs in #overlap-manager-root
function findPositionToolByPropertiesPanel(): PositionToolData | null {
  const overlayRoot = document.getElementById("overlap-manager-root");
  if (!overlayRoot) return null;

  const allLabels = overlayRoot.querySelectorAll("span, label, div");
  const labelValuePairs: { label: string; value: string }[] = [];

  for (const label of allLabels) {
    const text = label.textContent?.trim().toLowerCase() || "";
    if (!text) continue;

    const parent = label.closest("[class*='row'], [class*='cell'], [class*='group'], [class*='property']");
    if (!parent) continue;

    const input = parent.querySelector("input");
    if (input?.value) {
      labelValuePairs.push({ label: text, value: input.value });
    }
  }

  return extractFromLabelValues(labelValuePairs);
}

// Strategy 4: Look for the on-chart position tool overlay
// When a position tool is drawn, TradingView shows entry/stop/target lines with price labels
function findPositionToolByChartOverlay(): PositionToolData | null {
  // Position tools render directly on the chart canvas with HTML overlays for prices
  // Look for the characteristic green (profit) and red (loss) zones

  // TradingView renders position tools with specific data attributes
  const positionElements = document.querySelectorAll(
    '[data-name="long-position"], [data-name="short-position"], ' +
    '[class*="position-tool"], [class*="positionTool"]'
  );

  for (const el of positionElements) {
    const priceLabels = el.querySelectorAll('[class*="price"], [class*="value"], span');
    const prices: number[] = [];

    for (const label of priceLabels) {
      const parsed = parsePrice(label.textContent || "");
      if (parsed !== null && parsed > 0) {
        prices.push(parsed);
      }
    }

    if (prices.length >= 3) {
      return inferSideFromPrices(prices[0], prices[1], prices[2]);
    }
  }

  return null;
}

// Strategy 5: Scan all floating panels for price-like content
// Fallback when specific selectors fail
function findPositionToolByPriceScan(): PositionToolData | null {
  const overlayRoot = document.getElementById("overlap-manager-root");
  const containers = overlayRoot
    ? [overlayRoot, document.body]
    : [document.body];

  for (const container of containers) {
    // Look for dialogs/panels that contain position-related text
    const dialogs = container.querySelectorAll(
      '[data-name*="dialog"], [data-name*="properties"], ' +
      '[class*="dialog"], [class*="floating"], [class*="popover"]'
    );

    for (const dialog of dialogs) {
      const text = dialog.textContent?.toLowerCase() || "";
      // Must contain position-related keywords
      const hasEntry = text.includes("entry") || text.includes("price");
      const hasStop = text.includes("stop") || text.includes("loss");
      const hasTarget = text.includes("target") || text.includes("profit") || text.includes("take");

      if (hasEntry && hasStop && hasTarget) {
        return extractPricesFromDialog(dialog);
      }
    }
  }

  return null;
}

function extractPricesFromDialog(dialog: Element): PositionToolData | null {
  const inputs = dialog.querySelectorAll("input");
  const labels = dialog.querySelectorAll("span, label, div");

  const pairs: { label: string; value: string }[] = [];

  for (const label of labels) {
    const text = label.textContent?.trim().toLowerCase() || "";
    if (!text) continue;

    const row = label.closest("[class*='row'], [class*='cell'], [class*='group'], [class*='property'], tr, li");
    if (!row) continue;

    const input = row.querySelector("input");
    if (input?.value) {
      pairs.push({ label: text, value: input.value });
    }

    // Also check for plain text values (not in inputs)
    const sibling = label.nextElementSibling;
    if (sibling?.textContent) {
      const parsed = parsePrice(sibling.textContent);
      if (parsed !== null) {
        pairs.push({ label: text, value: sibling.textContent });
      }
    }
  }

  return extractFromLabelValues(pairs);
}

function extractFromLabelValues(pairs: { label: string; value: string }[]): PositionToolData | null {
  let entry: number | null = null;
  let stop: number | null = null;
  let target: number | null = null;

  for (const { label, value } of pairs) {
    const parsed = parsePrice(value);
    if (parsed === null) continue;

    if (label.includes("entry") || (label.includes("price") && !label.includes("stop") && !label.includes("target"))) {
      entry = parsed;
    } else if (label.includes("stop") || label.includes("loss")) {
      stop = parsed;
    } else if (label.includes("target") || label.includes("profit") || label.includes("take")) {
      target = parsed;
    }
  }

  if (entry !== null && stop !== null && target !== null) {
    return inferSideFromPrices(entry, stop, target);
  }
  return null;
}

function inferSideFromPrices(entry: number, stop: number, target: number): PositionToolData {
  // LONG: entry > stop, target > entry
  // SHORT: entry < stop, target < entry
  const side: "LONG" | "SHORT" = entry > stop ? "LONG" : "SHORT";
  return { entry, stop, target, side };
}

// --- Main Scraper Function ---

export function scrapeTradeSetup(strategiesOnly?: number[]): TradeSetup | null {
  // Try each strategy in order of reliability.
  // Dialog-based strategies (exact prices) take priority over Chart API (tick math).
  const allStrategies = [
    findPositionToolByDataName,       // Strategy 0: properties dialog data-name (exact prices)
    findPositionToolByPropertiesDialog, // Strategy 1: dialog by role (exact prices)
    findPositionToolByChartApi,       // Strategy 2: chart API (computed from tick math)
    findPositionToolByPropertiesPanel,  // Strategy 3: legacy overlay panel
    findPositionToolByChartOverlay,     // Strategy 4: chart overlay elements
    findPositionToolByPriceScan,        // Strategy 5: floating panel scan
  ];

  const indicesToTry = strategiesOnly ?? allStrategies.map((_, i) => i);

  for (const i of indicesToTry) {
    const strategy = allStrategies[i];
    if (!strategy) continue;
    try {
      const result = strategy();
      const parsedResult = validatePositionToolData(result);
      if (parsedResult.success) {
        const symbol = scrapeSymbol();
        if (!symbol) {
          console.warn("[Testudo] Found position tool but could not extract symbol");
          recordScraperResult(null);
          return null;
        }

        const timeframe = scrapeTimeframe();
        recordScraperResult(i);

        const tradeSetup = validateTradeSetup({
          symbol,
          side: parsedResult.data.side,
          entry: parsedResult.data.entry,
          stop: parsedResult.data.stop,
          target: parsedResult.data.target,
          timeframe,
        });

        if (!tradeSetup.success) {
          console.warn("[Testudo] Dropped malformed trade setup", tradeSetup.error);
          recordScraperResult(null);
          return null;
        }

        return tradeSetup.data;
      }
    } catch (err) {
      console.warn("[Testudo] Scraper strategy failed:", err);
    }
  }

  recordScraperResult(null);
  return null;
}

// --- Chart API Health Detection (FR-11) ---

export function getChartApiHealth(): ChartApiHealth {
  const w = window as any;
  const widget = w.TradingViewApi || w.ChartApiInstance;

  if (!widget) {
    return { available: false, hasActiveChart: false, hasGetAllShapes: false, hasGetShapeById: false };
  }

  const hasActiveChart = typeof widget.activeChart === "function";
  let hasGetAllShapes = false;
  let hasGetShapeById = false;

  if (hasActiveChart) {
    try {
      const chart = widget.activeChart();
      hasGetAllShapes = chart && typeof chart.getAllShapes === "function";
      hasGetShapeById = chart && typeof chart.getShapeById === "function";
    } catch { /* silent */ }
  }

  return { available: true, hasActiveChart, hasGetAllShapes, hasGetShapeById };
}

// --- Scraper Telemetry (FR-12) ---

let telemetryQueue: Promise<void> = Promise.resolve();

function recordScraperResult(strategyUsed: number | null): void {
  const record: ScraperHealthRecord = {
    timestamp: Date.now(),
    strategyUsed,
    success: strategyUsed !== null,
  };

  telemetryQueue = telemetryQueue.then(async () => {
    const stored = await browser.storage.local.get(["scraperHealth"]);
    const historyResult = validateHealthHistory(stored.scraperHealth);
    const history = historyResult.success ? historyResult.data : [];
    history.push(record);
    const trimmed = history.slice(-SCRAPER_HEALTH_MAX);
    await browser.storage.local.set({ scraperHealth: trimmed });
  }).catch(() => {});
}
