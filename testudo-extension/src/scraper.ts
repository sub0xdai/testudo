// EXT-02: TradingView DOM Scraper
// Extracts trade setup data from TradingView's Long/Short Position drawing tools.
// Uses multiple selector strategies with fallbacks for resilience.

export interface TradeSetup {
  symbol: string;
  side: "LONG" | "SHORT";
  entry: number;
  stop: number;
  target: number;
  timeframe: string;
}

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

function scrapeSymbol(): string | null {
  for (const selector of SYMBOL_SELECTORS) {
    const el = document.querySelector(selector);
    if (!el?.textContent) continue;

    // Extract symbol text, strip exchange prefix (e.g., "BINANCE:" or "BYBIT:")
    let text = el.textContent.trim();
    // Remove exchange prefix
    if (text.includes(":")) {
      text = text.split(":").pop()!.trim();
    }
    // Remove perpetual suffix (.P, PERP)
    text = text.replace(/\.P$/, "").replace(/PERP$/, "");
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

function scrapeTimeframe(): string {
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
// TradingView's Long/Short Position tools render a floating panel with price levels.
// The panel appears in #overlap-manager-root or as a direct child of the chart container.

interface PositionToolData {
  entry: number;
  stop: number;
  target: number;
  side: "LONG" | "SHORT";
}

// Strategy 1: Look for the position tool's floating properties panel
// The panel contains labeled rows with Entry, Stop, Target (or Profit) values
function findPositionToolByPropertiesPanel(): PositionToolData | null {
  // The properties dialog appears in the overlap manager
  const overlayRoot = document.getElementById("overlap-manager-root");
  if (!overlayRoot) return null;

  // Look for dialog/panel with position-related inputs
  const allInputs = overlayRoot.querySelectorAll("input");
  const allLabels = overlayRoot.querySelectorAll("span, label, div");

  // Build a map of label → value pairs
  const labelValuePairs: { label: string; value: string }[] = [];

  for (const label of allLabels) {
    const text = label.textContent?.trim().toLowerCase() || "";
    if (!text) continue;

    // Check if this label is near an input
    const parent = label.closest("[class*='row'], [class*='cell'], [class*='group'], [class*='property']");
    if (!parent) continue;

    const input = parent.querySelector("input");
    if (input?.value) {
      labelValuePairs.push({ label: text, value: input.value });
    }
  }

  return extractFromLabelValues(labelValuePairs);
}

// Strategy 2: Look for the on-chart position tool overlay
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

// Strategy 3: Scan all floating panels for price-like content
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

export function scrapeTradeSetup(): TradeSetup | null {
  // Try each strategy in order of reliability
  const strategies = [
    findPositionToolByPropertiesPanel,
    findPositionToolByChartOverlay,
    findPositionToolByPriceScan,
  ];

  for (const strategy of strategies) {
    try {
      const result = strategy();
      if (result) {
        const symbol = scrapeSymbol();
        if (!symbol) {
          console.warn("[Testudo] Found position tool but could not extract symbol");
          return null;
        }

        const timeframe = scrapeTimeframe();

        return {
          symbol,
          side: result.side,
          entry: result.entry,
          stop: result.stop,
          target: result.target,
          timeframe,
        };
      }
    } catch (err) {
      console.warn("[Testudo] Scraper strategy failed:", err);
    }
  }

  return null;
}

// --- MutationObserver for Tool Detection ---

let observer: MutationObserver | null = null;
let onToolDetected: ((setup: TradeSetup) => void) | null = null;

export function startWatching(callback: (setup: TradeSetup) => void): void {
  stopWatching();
  onToolDetected = callback;

  observer = new MutationObserver(() => {
    const setup = scrapeTradeSetup();
    if (setup && onToolDetected) {
      onToolDetected(setup);
    }
  });

  // Watch the overlay root for position tool dialogs
  const overlayRoot = document.getElementById("overlap-manager-root");
  if (overlayRoot) {
    observer.observe(overlayRoot, { childList: true, subtree: true });
  }

  // Also watch the chart container for on-chart position tools
  const chartContainer = document.querySelector('[class*="chart-container"], [class*="chartContainer"]');
  if (chartContainer) {
    observer.observe(chartContainer, { childList: true, subtree: true });
  }
}

export function stopWatching(): void {
  observer?.disconnect();
  observer = null;
  onToolDetected = null;
}
