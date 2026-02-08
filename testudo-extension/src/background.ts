import browser from "webextension-polyfill";

// Background service worker — manages settings, REST dispatch, and connection state.
// EXT-05 will add JWT token refresh here.
// EXT-06 will add WebSocket connection management here.

interface Settings {
  backendUrl: string;
  executionMode: "paper" | "live";
}

const DEFAULT_SETTINGS: Settings = {
  backendUrl: "http://localhost:8080",
  executionMode: "paper",
};

// Default paper trading user ID (auto-initialized by backend)
const PAPER_USER_ID = "00000000-0000-0000-0000-000000000001";

async function getSettings(): Promise<Settings> {
  const stored = await browser.storage.local.get(["backendUrl", "executionMode"]);
  return {
    backendUrl: (stored.backendUrl as string) || DEFAULT_SETTINGS.backendUrl,
    executionMode: (stored.executionMode as Settings["executionMode"]) || DEFAULT_SETTINGS.executionMode,
  };
}

browser.runtime.onInstalled.addListener(async () => {
  const settings = await getSettings();
  await browser.storage.local.set({ ...settings });
  console.log("Testudo Sniper installed", settings);
});

// --- Symbol Normalization (EXT-04 FR-3) ---

const QUOTE_CURRENCIES = [
  "USDT", "USDC", "BUSD", "TUSD", "FDUSD",
  "BTC", "ETH", "BNB", "DAI",
  "EUR", "GBP", "USD",
];

function normalizeSymbol(tvSymbol: string): string {
  // Input: TradingView symbol already cleaned by scraper (no exchange prefix, no .P suffix)
  // Output: backend format "BASE_QUOTE" (e.g., "BTC_USDT")
  const upper = tvSymbol.toUpperCase();

  for (const quote of QUOTE_CURRENCIES) {
    if (upper.endsWith(quote) && upper.length > quote.length) {
      const base = upper.slice(0, -quote.length);
      return `${base}_${quote}`;
    }
  }

  // Fallback: return as-is if no known quote currency found
  return upper;
}

// --- Trade Execution (EXT-04 FR-1, FR-2, FR-4, FR-5, FR-6) ---

interface TradePayload {
  symbol: string;
  side: "LONG" | "SHORT";
  entry: number;
  stop: number;
  target: number;
  timeframe: string;
}

interface BackendResponse {
  success: boolean;
  data?: unknown;
  error?: string | null;
}

async function executeTrade(payload: TradePayload): Promise<BackendResponse> {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/trades`;

  // Map side: LONG → buy, SHORT → sell
  const side = payload.side === "LONG" ? "buy" : "sell";

  // Calculate basic position size: risk_amount / stop_distance
  // Default: 1% risk on 10,000 USDT paper balance = 100 USDT risk
  const stopDistance = Math.abs(payload.entry - payload.stop);
  const riskAmount = 100;
  const quantity = stopDistance > 0 ? riskAmount / stopDistance : 0.001;

  // Round to reasonable precision (8 decimal places)
  const roundedQty = Math.round(quantity * 1e8) / 1e8;

  const body = {
    symbol: normalizeSymbol(payload.symbol),
    side,
    quantity: roundedQty.toString(),
    entry_price: payload.entry.toString(),
    stop_loss_price: payload.stop.toString(),
    take_profit_price: payload.target.toString(),
  };

  try {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-User-Id": PAPER_USER_ID,
      },
      body: JSON.stringify(body),
    });

    const json = await response.json() as BackendResponse;

    if (!response.ok) {
      return {
        success: false,
        error: json.error || `HTTP ${response.status}`,
      };
    }

    return json;
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

// --- Message Router ---

browser.runtime.onMessage.addListener((message: unknown) => {
  const msg = message as { type: string; payload?: TradePayload };

  if (msg.type === "GET_SETTINGS") {
    return getSettings();
  }

  if (msg.type === "EXECUTE_TRADE" && msg.payload) {
    return executeTrade(msg.payload);
  }
});
