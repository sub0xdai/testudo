import browser from "webextension-polyfill";

// Background service worker — manages settings, auth, REST dispatch, and connection state.
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

// --- Auth Token Management (EXT-05 FR-2, FR-3, FR-7) ---

interface AuthTokens {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

interface LoginResponse {
  user: { id: string; email: string };
  tokens: AuthTokens;
}

async function getTokens(): Promise<AuthTokens | null> {
  const stored = await browser.storage.local.get(["accessToken", "refreshToken", "tokenExpiry"]);
  if (!stored.accessToken || !stored.refreshToken) return null;
  return {
    access_token: stored.accessToken as string,
    refresh_token: stored.refreshToken as string,
    expires_in: ((stored.tokenExpiry as number) || 0) - Math.floor(Date.now() / 1000),
  };
}

async function storeTokens(tokens: AuthTokens): Promise<void> {
  await browser.storage.local.set({
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token,
    tokenExpiry: Math.floor(Date.now() / 1000) + tokens.expires_in,
  });
}

async function clearTokens(): Promise<void> {
  await browser.storage.local.remove(["accessToken", "refreshToken", "tokenExpiry"]);
}

async function login(email: string, password: string): Promise<{ success: boolean; error?: string }> {
  const settings = await getSettings();
  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/auth/login`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });

    if (!response.ok) {
      const json = await response.json() as { error?: string; message?: string };
      return { success: false, error: json.message || json.error || `HTTP ${response.status}` };
    }

    const json = await response.json() as LoginResponse;
    await storeTokens(json.tokens);
    scheduleTokenRefresh(json.tokens.expires_in);
    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Login failed";
    return { success: false, error: msg };
  }
}

async function refreshAccessToken(): Promise<boolean> {
  const tokens = await getTokens();
  if (!tokens) return false;

  const settings = await getSettings();
  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/auth/refresh`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ refresh_token: tokens.refresh_token }),
    });

    if (!response.ok) {
      await clearTokens();
      return false;
    }

    const json = await response.json() as { tokens: AuthTokens };
    await storeTokens(json.tokens);
    scheduleTokenRefresh(json.tokens.expires_in);
    return true;
  } catch {
    return false;
  }
}

let refreshTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleTokenRefresh(expiresIn: number): void {
  if (refreshTimer) clearTimeout(refreshTimer);
  // Refresh 60 seconds before expiry, minimum 10 seconds
  const refreshDelay = Math.max(10, expiresIn - 60) * 1000;
  refreshTimer = setTimeout(() => {
    refreshAccessToken();
  }, refreshDelay);
}

async function getAuthStatus(): Promise<{ authenticated: boolean; email?: string }> {
  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { authenticated: false };
  }
  // Decode email from access token (JWT payload)
  try {
    const payload = JSON.parse(atob(tokens.access_token.split(".")[1])) as { email?: string };
    return { authenticated: true, email: payload.email };
  } catch {
    return { authenticated: true };
  }
}

// --- Symbol Normalization (EXT-04 FR-3) ---

const QUOTE_CURRENCIES = [
  "USDT", "USDC", "BUSD", "TUSD", "FDUSD",
  "BTC", "ETH", "BNB", "DAI",
  "EUR", "GBP", "USD",
];

function normalizeSymbol(tvSymbol: string): string {
  const upper = tvSymbol.toUpperCase();
  for (const quote of QUOTE_CURRENCIES) {
    if (upper.endsWith(quote) && upper.length > quote.length) {
      const base = upper.slice(0, -quote.length);
      return `${base}_${quote}`;
    }
  }
  return upper;
}

// --- Trade Execution (EXT-04 + EXT-05 FR-5, FR-7) ---

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

  const side = payload.side === "LONG" ? "buy" : "sell";

  // Position size: risk_amount / stop_distance (1% of 10k default)
  const stopDistance = Math.abs(payload.entry - payload.stop);
  const riskAmount = 100;
  const quantity = stopDistance > 0 ? riskAmount / stopDistance : 0.001;
  const roundedQty = Math.round(quantity * 1e8) / 1e8;

  const body = {
    symbol: normalizeSymbol(payload.symbol),
    side,
    quantity: roundedQty.toString(),
    entry_price: payload.entry.toString(),
    stop_loss_price: payload.stop.toString(),
    take_profit_price: payload.target.toString(),
  };

  // EXT-05 FR-7: Build headers — prefer JWT Bearer token, fall back to X-User-Id
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  } else {
    headers["X-User-Id"] = PAPER_USER_ID;
  }

  // EXT-05 FR-5: Send execution mode header
  headers["X-Execution-Mode"] = settings.executionMode;

  try {
    const response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
    });

    const json = await response.json() as BackendResponse;

    if (!response.ok) {
      // If 401, try refreshing token and retry once
      if (response.status === 401 && tokens) {
        const refreshed = await refreshAccessToken();
        if (refreshed) {
          return executeTrade(payload);
        }
      }
      return { success: false, error: json.error || `HTTP ${response.status}` };
    }

    return json;
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

// --- Message Router ---

type Message =
  | { type: "GET_SETTINGS" }
  | { type: "EXECUTE_TRADE"; payload: TradePayload }
  | { type: "LOGIN"; email: string; password: string }
  | { type: "LOGOUT" }
  | { type: "AUTH_STATUS" }
  | { type: "REFRESH_TOKEN" };

browser.runtime.onMessage.addListener((message: unknown) => {
  const msg = message as Message;

  if (msg.type === "GET_SETTINGS") {
    return getSettings();
  }

  if (msg.type === "EXECUTE_TRADE" && "payload" in msg) {
    return executeTrade(msg.payload);
  }

  if (msg.type === "LOGIN" && "email" in msg && "password" in msg) {
    return login(msg.email, msg.password);
  }

  if (msg.type === "LOGOUT") {
    return clearTokens().then(() => ({ success: true }));
  }

  if (msg.type === "AUTH_STATUS") {
    return getAuthStatus();
  }

  if (msg.type === "REFRESH_TOKEN") {
    return refreshAccessToken().then((ok) => ({ success: ok }));
  }
});

// On startup, schedule token refresh if tokens exist
getTokens().then((tokens) => {
  if (tokens && tokens.expires_in > 0) {
    scheduleTokenRefresh(tokens.expires_in);
  }
});
