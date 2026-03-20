import browser from "webextension-polyfill";
import type { Settings, AuthTokens, LoginResponse, TradePayload, BackendResponse, WsState, TradeGroupResponse, BalanceResponse, LiveBalanceResponse, ExchangeInfo, ExchangeAccount, AddExchangeAccountPayload, TestConnectionResult, ExchangePositionsResponse } from "./types";
import {
  DEFAULT_SETTINGS, WS_BASE_RECONNECT_DELAY,
  normalizeSymbol, mapSide, calculateRefreshDelay, nextReconnectDelay,
  getExchangeType,
} from "./utils";
import type { ExchangeMode } from "./utils";
import {
  AddExchangeAccountResponseSchema,
  AuthTokensSchema,
  BackendResponseSchema,
  ErrorResponseSchema,
  ExchangeAccountsResponseSchema,
  ExchangeBalanceApiResponseSchema,
  ExchangePositionsApiResponseSchema,
  ListExchangesResponseSchema,
  LoginResponseSchema,
  JwtEmailPayloadSchema,
  JwtSubPayloadSchema,
  RefreshResponseSchema,
  RuntimeMessageSchema,
  SettingsSchema,
  SidecarHealthResponseSchema,
  SidecarStreamDataSchema,
  StoredSettingsSchema,
  StoredTokensSchema,
  TestConnectionResultSchema,
  TradeGroupResponseSchema,
  TradeListResponseSchema,
  WebSocketMessageSchema,
} from "./schemas";

// Background service worker — manages settings, auth, REST dispatch, and WebSocket connection.

// AUD-07 FR-8: Global error logging for unhandled promise rejections
self.addEventListener("unhandledrejection", (event: PromiseRejectionEvent) => {
  console.error("Unhandled promise rejection:", event.reason);
});

type RuntimeTradePayload = Omit<TradePayload, "management"> & {
  management: Omit<TradePayload["management"], "leverage"> & { leverage?: number };
};

function normalizeBackendAck(raw: unknown): BackendResponse {
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    // Extract warnings from nested data if present
    const dataObj = obj.data && typeof obj.data === "object" ? obj.data as Record<string, unknown> : null;
    const warnings = Array.isArray(dataObj?.warnings) ? dataObj.warnings as string[] : undefined;

    if (typeof obj.success === "boolean") {
      return {
        success: obj.success,
        data: obj.data,
        error: typeof obj.error === "string" || obj.error === null ? obj.error : null,
        warnings,
      };
    }

    if (typeof obj.error === "string") {
      return { success: false, data: null, error: obj.error };
    }

    if (typeof obj.message === "string") {
      return { success: false, data: null, error: obj.message };
    }

    return { success: true, data: raw, error: null, warnings };
  }

  return { success: true, data: raw, error: null };
}

function normalizeTradeListResponse(raw: unknown): BackendResponse {
  const normalizeTradeArray = (value: unknown): TradeGroupResponse[] | null => {
    if (!Array.isArray(value)) return null;
    const parsed: TradeGroupResponse[] = [];
    for (const item of value) {
      const trade = TradeGroupResponseSchema.safeParse(item);
      if (trade.success) parsed.push(trade.data);
    }
    if (value.length > 0 && parsed.length === 0) {
      return null;
    }
    return parsed;
  };

  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;

    if (typeof obj.success === "boolean") {
      if (!obj.success) {
        let error = "Trade list request failed";
        if (typeof obj.error === "string") {
          error = obj.error;
        } else if (typeof obj.message === "string") {
          error = obj.message;
        }
        return { success: false, data: null, error };
      }

      const direct = normalizeTradeArray(obj.data);
      if (direct) return { success: true, data: direct, error: null };

      if (obj.data && typeof obj.data === "object") {
        const nested = normalizeTradeArray((obj.data as Record<string, unknown>).trades);
        if (nested) return { success: true, data: nested, error: null };
      }

      const topLevel = normalizeTradeArray(obj.trades);
      if (topLevel) return { success: true, data: topLevel, error: null };
    }

    const fromData = normalizeTradeArray(obj.data);
    if (fromData) return { success: true, data: fromData, error: null };

    const fromTrades = normalizeTradeArray(obj.trades);
    if (fromTrades) return { success: true, data: fromTrades, error: null };

    if (obj.data && typeof obj.data === "object") {
      const nested = normalizeTradeArray((obj.data as Record<string, unknown>).trades);
      if (nested) return { success: true, data: nested, error: null };
    }

    if (typeof obj.error === "string") {
      return { success: false, data: null, error: obj.error };
    }

    if (typeof obj.message === "string") {
      return { success: false, data: null, error: obj.message };
    }
  }

  const direct = normalizeTradeArray(raw);
  if (direct) {
    return { success: true, data: direct, error: null };
  }

  return { success: false, data: null, error: "Malformed trade list response" };
}

async function getSettings(): Promise<Settings> {
  const stored = await browser.storage.local.get(["backendUrl", "wsUrl"]);
  const parsed = StoredSettingsSchema.safeParse(stored);

  if (!parsed.success) {
    return { ...DEFAULT_SETTINGS };
  }

  const candidate = {
    backendUrl: parsed.data.backendUrl || DEFAULT_SETTINGS.backendUrl,
    wsUrl: parsed.data.wsUrl || DEFAULT_SETTINGS.wsUrl,
  };

  const validated = SettingsSchema.safeParse(candidate);
  if (!validated.success) {
    return { ...DEFAULT_SETTINGS };
  }

  return validated.data;
}

browser.runtime.onInstalled.addListener(async () => {
  const settings = await getSettings();
  // EXT-19: Clean up legacy paper trading storage keys
  await browser.storage.local.remove(["executionMode", "paperOnly"]);
  await browser.storage.local.set({ ...settings });
  console.log("Testudo Sniper installed", settings);
});

// --- Auth Token Management (EXT-05 FR-2, FR-3, FR-7) ---

async function getTokens(): Promise<AuthTokens | null> {
  const stored = await browser.storage.local.get(["accessToken", "refreshToken", "tokenExpiry"]);
  const parsed = StoredTokensSchema.safeParse(stored);
  if (!parsed.success) return null;

  const tokens = {
    access_token: parsed.data.accessToken,
    refresh_token: parsed.data.refreshToken,
    expires_in: (parsed.data.tokenExpiry || 0) - Math.floor(Date.now() / 1000),
  };

  const validated = AuthTokensSchema.safeParse(tokens);
  return validated.success ? validated.data : null;
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
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      if (!json.success) {
        return { success: false, error: `HTTP ${response.status}` };
      }
      return { success: false, error: json.data.message || json.data.error || `HTTP ${response.status}` };
    }

    const parsed = LoginResponseSchema.safeParse(await response.json());
    if (!parsed.success) {
      return { success: false, error: "Unexpected server response" };
    }
    await storeTokens(parsed.data.tokens);
    scheduleTokenRefresh(parsed.data.tokens.expires_in);
    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Login failed";
    return { success: false, error: msg };
  }
}

let refreshInFlight: Promise<boolean> | null = null;

async function refreshAccessToken(): Promise<boolean> {
  if (refreshInFlight) return refreshInFlight;

  refreshInFlight = doRefresh();
  try {
    return await refreshInFlight;
  } finally {
    refreshInFlight = null;
  }
}

async function doRefresh(): Promise<boolean> {
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

    const parsed = RefreshResponseSchema.safeParse(await response.json());
    if (!parsed.success) return false;
    await storeTokens(parsed.data.tokens);
    scheduleTokenRefresh(parsed.data.tokens.expires_in);
    return true;
  } catch {
    return false;
  }
}

let refreshTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleTokenRefresh(expiresIn: number): void {
  if (refreshTimer) clearTimeout(refreshTimer);
  const refreshDelay = calculateRefreshDelay(expiresIn);
  refreshTimer = setTimeout(() => {
    refreshAccessToken();
  }, refreshDelay);
}

async function getAuthStatus(): Promise<{ authenticated: boolean; email?: string }> {
  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { authenticated: false };
  }
  try {
    const payloadRaw = JSON.parse(atob(tokens.access_token.split(".")[1] || ""));
    const payload = JwtEmailPayloadSchema.safeParse(payloadRaw);
    if (!payload.success) {
      return { authenticated: true };
    }
    return { authenticated: true, email: payload.data.email };
  } catch {
    return { authenticated: true };
  }
}

// --- Active Exchange Selection (EXT-32: per-mode active exchange) ---

async function getExchangeMode(): Promise<ExchangeMode> {
  const stored = await browser.storage.local.get("exchangeMode");
  const mode = stored.exchangeMode;
  return mode === "dex" ? "dex" : "cex";
}

async function getActiveExchangeId(): Promise<string | null> {
  const mode = await getExchangeMode();
  const key = mode === "dex" ? "activeDexAccountId" : "activeCexAccountId";
  const stored = await browser.storage.local.get(key);
  return (stored[key] as string) || null;
}

async function setActiveExchangeId(id: string | null): Promise<void> {
  const mode = await getExchangeMode();
  const key = mode === "dex" ? "activeDexAccountId" : "activeCexAccountId";
  if (id) {
    await browser.storage.local.set({ [key]: id });
  } else {
    await browser.storage.local.remove([key]);
  }
}

async function ensureActiveExchange(): Promise<string | null> {
  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) return null;

  const mode = await getExchangeMode();
  const currentId = await getActiveExchangeId();
  const result = await listExchangeAccounts();

  // Don't clear active ID if the API call failed — preserve what we have
  if (!result.success) return currentId;

  const allAccounts = result.data || [];
  const accounts = allAccounts.filter((a) => getExchangeType(a.exchange_name) === mode);

  if (accounts.length === 0) {
    if (currentId) await setActiveExchangeId(null);
    return null;
  }

  if (currentId && accounts.some((a) => a.id === currentId)) {
    return currentId;
  }

  const firstId = accounts[0].id;
  await setActiveExchangeId(firstId);
  return firstId;
}

// One-time migration from legacy activeExchangeId to per-mode keys
async function migrateActiveExchangeId(): Promise<void> {
  const stored = await browser.storage.local.get([
    "activeExchangeId", "activeCexAccountId", "activeDexAccountId",
  ]);
  const legacy = stored.activeExchangeId as string | undefined;
  if (!legacy || stored.activeCexAccountId || stored.activeDexAccountId) return;

  const result = await listExchangeAccounts();
  const accounts = result.success ? (result.data || []) : [];
  const account = accounts.find((a) => a.id === legacy);
  const type = account ? getExchangeType(account.exchange_name) : "cex";
  const key = type === "dex" ? "activeDexAccountId" : "activeCexAccountId";
  await browser.storage.local.set({ [key]: legacy, exchangeMode: type });
  await browser.storage.local.remove("activeExchangeId");
}

// --- Trade Execution (EXT-19: live only, JWT required) ---

let tradeInFlight = false;

async function executeTrade(payload: RuntimeTradePayload, retried = false): Promise<BackendResponse> {
  if (tradeInFlight && !retried) {
    return { success: false, error: "Trade already in progress" };
  }
  if (!retried) tradeInFlight = true;

  try {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/trades`;

  const activeExchangeId = payload.exchange_account_id || await getActiveExchangeId();

  const body: Record<string, unknown> = {
    symbol: normalizeSymbol(payload.symbol),
    side: mapSide(payload.side),
    entry_price: payload.entry.toString(),
    stop_loss_price: payload.stop.toString(),
    take_profit_price: payload.target.toString(),
    management: payload.management,
  };

  if (activeExchangeId) {
    body.exchange_account_id = activeExchangeId;
  }

  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { success: false, error: "Authentication required — please log in" };
  }

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    "Authorization": `Bearer ${tokens.access_token}`,
  };

  try {
    const response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      const raw = await response.json().catch(() => ({}));
      console.error("[executeTrade] HTTP error:", response.status, raw);
      const json = ErrorResponseSchema.safeParse(raw);

      if (response.status === 401 && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) {
          return executeTrade(payload, true);
        }
      }
      const errorMsg = json.success ? (json.data.error || json.data.message) : undefined;
      return { success: false, error: errorMsg || `HTTP ${response.status}` };
    }

    const raw = await response.json().catch(() => ({}));
    console.log("[executeTrade] response:", JSON.stringify(raw).slice(0, 500));
    const normalized = normalizeBackendAck(raw);
    const validated = BackendResponseSchema.safeParse(normalized);
    if (!validated.success) {
      console.error("[executeTrade] validation failed:", validated.error.message);
      return { success: false, error: "Malformed trade response" };
    }
    return validated.data;
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
  } finally {
    if (!retried) tradeInFlight = false;
  }
}

// --- Trade Listing (Active Orders) ---

async function listTrades(retried = false): Promise<{ success: boolean; data?: TradeGroupResponse[]; error?: string }> {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/trades`;

  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { success: false, error: "Authentication required" };
  }

  const headers: Record<string, string> = {
    "Authorization": `Bearer ${tokens.access_token}`,
  };

  try {
    const response = await fetch(url, { headers });
    const raw = await response.json().catch(() => ({}));
    const normalized = normalizeTradeListResponse(raw);

    if (!response.ok) {
      if (response.status === 401 && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return listTrades(true);
      }
      const msg = normalized.error || `HTTP ${response.status}`;
      return { success: false, error: msg };
    }

    const validated = TradeListResponseSchema.safeParse(normalized);
    if (!validated.success) {
      return { success: false, error: "Malformed trade list response" };
    }

    if (!validated.data.success) {
      return { success: false, error: validated.data.error || "Trade list request failed" };
    }

    return { success: true, data: validated.data.data || [] };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

async function cancelTrade(tradeId: string, retried = false): Promise<BackendResponse> {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/trades/${tradeId}`;

  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { success: false, error: "Authentication required" };
  }

  const headers: Record<string, string> = {
    "Authorization": `Bearer ${tokens.access_token}`,
  };

  try {
    const response = await fetch(url, { method: "DELETE", headers });
    if (!response.ok) {
      if (response.status === 401 && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return cancelTrade(tradeId, true);
      }
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      const errorMsg = json.success ? json.data.error : undefined;
      return { success: false, error: errorMsg || `HTTP ${response.status}` };
    }

    const raw = await response.json().catch(() => ({}));
    const normalized = normalizeBackendAck(raw);
    const validated = BackendResponseSchema.safeParse(normalized);
    if (!validated.success) {
      return { success: false, error: "Malformed cancel response" };
    }
    return validated.data;
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

async function cleanupTrades(retried = false): Promise<BackendResponse> {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/trades/cleanup`;

  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { success: false, error: "Authentication required" };
  }

  try {
    const response = await fetch(url, {
      method: "POST",
      headers: { "Authorization": `Bearer ${tokens.access_token}` },
    });
    if (!response.ok) {
      if (response.status === 401 && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return cleanupTrades(true);
      }
      return { success: false, error: `HTTP ${response.status}` };
    }
    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

// --- Registration (EXT-15 FR-2) ---

async function register(email: string, password: string): Promise<{ success: boolean; error?: string }> {
  const settings = await getSettings();
  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/auth/register`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email, password }),
    });

    if (!response.ok) {
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      if (!json.success) {
        return { success: false, error: `HTTP ${response.status}` };
      }
      return { success: false, error: json.data.message || json.data.error || `HTTP ${response.status}` };
    }

    const parsed = LoginResponseSchema.safeParse(await response.json());
    if (!parsed.success) {
      return { success: false, error: "Unexpected server response" };
    }
    await storeTokens(parsed.data.tokens);
    scheduleTokenRefresh(parsed.data.tokens.expires_in);
    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Registration failed";
    return { success: false, error: msg };
  }
}

// --- Password Reset (AUD-08 FR-5) ---

async function forgotPassword(email: string): Promise<{ success: boolean; error?: string }> {
  const settings = await getSettings();
  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/auth/forgot-password`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ email }),
    });

    if (!response.ok) {
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      if (!json.success) {
        return { success: false, error: `HTTP ${response.status}` };
      }
      return { success: false, error: json.data.message || json.data.error || `HTTP ${response.status}` };
    }

    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Request failed";
    return { success: false, error: msg };
  }
}

// --- Exchange Account Management (EXT-15 FR-4) ---

async function listExchanges(retried = false): Promise<{ success: boolean; data?: ExchangeInfo[]; error?: string }> {
  const settings = await getSettings();
  const headers: Record<string, string> = {};
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  }

  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/exchanges`, { headers });
    const raw = await response.json().catch(() => ({}));
    const json = ListExchangesResponseSchema.safeParse(raw);
    if (!json.success) {
      return { success: false, error: "Malformed exchanges response" };
    }

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return listExchanges(true);
      }
      return { success: false, error: json.data.error || `HTTP ${response.status}` };
    }

    return { success: true, data: json.data.exchanges || [] };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

async function listExchangeAccounts(retried = false): Promise<{ success: boolean; data?: ExchangeAccount[]; error?: string }> {
  const settings = await getSettings();
  const headers: Record<string, string> = {};
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  }

  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/exchanges/accounts`, { headers });
    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return listExchangeAccounts(true);
      }
      const errRaw = await response.json().catch(() => ({}));
      const errJson = ErrorResponseSchema.safeParse(errRaw);
      return { success: false, error: (errJson.success && errJson.data.error) || `HTTP ${response.status}` };
    }

    const raw = await response.json().catch(() => ([]));
    console.log("[listExchangeAccounts] raw response:", JSON.stringify(raw).slice(0, 500));
    const json = ExchangeAccountsResponseSchema.safeParse(raw);
    if (!json.success) {
      console.error("[listExchangeAccounts] schema parse failed:", json.error.issues);
      return { success: false, error: "Malformed exchange accounts response" };
    }
    const accounts = Array.isArray(json.data) ? json.data : (json.data.data || json.data.accounts || []);
    console.log("[listExchangeAccounts] parsed accounts:", accounts.length, accounts.map(a => a.exchange_name));
    return { success: true, data: accounts };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

async function addExchangeAccount(payload: AddExchangeAccountPayload, retried = false): Promise<{ success: boolean; data?: ExchangeAccount; error?: string }> {
  const settings = await getSettings();
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  }

  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/exchanges/accounts`, {
      method: "POST",
      headers,
      body: JSON.stringify(payload),
    });
    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return addExchangeAccount(payload, true);
      }
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      const errorMsg = json.success ? json.data.error : undefined;
      return { success: false, error: errorMsg || `HTTP ${response.status}` };
    }

    const raw = await response.json().catch(() => ({}));
    const json = AddExchangeAccountResponseSchema.safeParse(raw);
    if (!json.success) {
      return { success: false, error: "Malformed add account response" };
    }
    return { success: true, data: json.data.data };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

async function deleteExchangeAccount(accountId: string, retried = false): Promise<{ success: boolean; error?: string }> {
  const settings = await getSettings();
  const headers: Record<string, string> = {};
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  }

  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/exchanges/accounts/${accountId}`, {
      method: "DELETE",
      headers,
    });

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return deleteExchangeAccount(accountId, true);
      }
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      const errorMsg = json.success ? json.data.error : undefined;
      return { success: false, error: errorMsg || `HTTP ${response.status}` };
    }

    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

async function testExchangeConnection(accountId: string, retried = false): Promise<{ success: boolean; data?: TestConnectionResult; error?: string }> {
  const settings = await getSettings();
  const headers: Record<string, string> = {};
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  }

  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/exchanges/accounts/${accountId}/test`, {
      method: "POST",
      headers,
    });
    const raw = await response.json().catch(() => ({}));

    if (!response.ok) {
      const errJson = ErrorResponseSchema.safeParse(raw);
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return testExchangeConnection(accountId, true);
      }
      return { success: false, error: (errJson.success && errJson.data.error) || `HTTP ${response.status}` };
    }

    const json = TestConnectionResultSchema.safeParse(raw);
    if (!json.success) {
      return { success: false, error: "Malformed connection test response" };
    }

    return { success: true, data: json.data };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

// --- EXT-19: Live Exchange Balance (always from active exchange) ---

async function getLiveBalance(retried = false): Promise<{ success: boolean; data?: LiveBalanceResponse; error?: string }> {
  let activeId = await getActiveExchangeId();
  if (!activeId) {
    activeId = await ensureActiveExchange();
    if (!activeId) {
      return { success: false, error: "No active exchange selected" };
    }
  }

  const settings = await getSettings();
  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { success: false, error: "Authentication required for live balance" };
  }

  const headers: Record<string, string> = {
    "Authorization": `Bearer ${tokens.access_token}`,
  };

  try {
    const response = await fetch(
      `${settings.backendUrl}/api/v1/exchanges/accounts/${activeId}/balance`,
      { headers, signal: AbortSignal.timeout(10000) },
    );

    if (!response.ok) {
      if (response.status === 401 && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return getLiveBalance(true);
      }
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      const errorMsg = json.success ? (json.data.message || json.data.error) : undefined;
      return { success: false, error: errorMsg || `HTTP ${response.status}` };
    }

    const raw = await response.json().catch(() => ({}));
    const json = ExchangeBalanceApiResponseSchema.safeParse(raw);
    if (!json.success) {
      return { success: false, error: "Malformed balance response" };
    }

    const balances: BalanceResponse[] = json.data.balances.map((b) => ({
      asset: b.asset,
      available: b.free,
      locked: b.used,
    }));

    return {
      success: true,
      data: {
        exchange_name: json.data.exchange_name,
        balances,
      },
    };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

// --- Exchange Positions (live fallback) ---

async function fetchExchangePositions(retried = false): Promise<{ success: boolean; data?: ExchangePositionsResponse; error?: string }> {
  let activeId = await getActiveExchangeId();
  if (!activeId) {
    activeId = await ensureActiveExchange();
    if (!activeId) {
      return { success: false, error: "No active exchange selected" };
    }
  }

  const settings = await getSettings();
  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { success: false, error: "Authentication required" };
  }

  const headers: Record<string, string> = {
    "Authorization": `Bearer ${tokens.access_token}`,
  };

  try {
    const response = await fetch(
      `${settings.backendUrl}/api/v1/exchanges/accounts/${activeId}/positions`,
      { headers, signal: AbortSignal.timeout(15000) },
    );

    if (!response.ok) {
      if (response.status === 401 && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return fetchExchangePositions(true);
      }
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      const errorMsg = json.success ? (json.data.message || json.data.error) : undefined;
      return { success: false, error: errorMsg || `HTTP ${response.status}` };
    }

    const raw = await response.json().catch(() => ({}));
    const json = ExchangePositionsApiResponseSchema.safeParse(raw);
    if (!json.success) {
      return { success: false, error: "Malformed positions response" };
    }

    return { success: true, data: json.data };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

// --- EXT-34: Close Exchange Position ---

async function closeExchangePosition(
  symbol: string,
  side: string,
  contracts: string,
  retried = false,
): Promise<{ success: boolean; error?: string }> {
  let activeId = await getActiveExchangeId();
  if (!activeId) {
    activeId = await ensureActiveExchange();
    if (!activeId) {
      return { success: false, error: "No active exchange selected" };
    }
  }

  const settings = await getSettings();
  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { success: false, error: "Authentication required" };
  }

  try {
    const response = await fetch(
      `${settings.backendUrl}/api/v1/exchanges/accounts/${activeId}/close-position`,
      {
        method: "POST",
        headers: {
          "Authorization": `Bearer ${tokens.access_token}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ symbol, side, contracts }),
        signal: AbortSignal.timeout(30000),
      },
    );

    if (!response.ok) {
      if (response.status === 401 && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return closeExchangePosition(symbol, side, contracts, true);
      }
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      const errorMsg = json.success ? (json.data.message || json.data.error) : undefined;
      return { success: false, error: errorMsg || `HTTP ${response.status}` };
    }

    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

// --- Sidecar Health Polling (EXT-16 FR-2) ---

export type SidecarStatus = "unknown" | "healthy" | "unreachable";
let sidecarStatus: SidecarStatus = "unknown";

function setSidecarStatus(status: SidecarStatus): void {
  if (status === sidecarStatus) return;
  sidecarStatus = status;
  browser.runtime.sendMessage({ type: "SIDECAR_STATUS_CHANGED", status }).catch(() => {});
}

async function checkSidecarHealth(): Promise<void> {
  const settings = await getSettings();
  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/health/sidecar`, {
      signal: AbortSignal.timeout(5000),
    });
    if (response.ok) {
      const raw = await response.json().catch(() => ({}));
      const json = SidecarHealthResponseSchema.safeParse(raw);
      setSidecarStatus(json.success && json.data.status === "healthy" ? "healthy" : "unreachable");
    } else {
      setSidecarStatus("unreachable");
    }
  } catch {
    setSidecarStatus("unreachable");
  }
}

let sidecarHealthTimer: ReturnType<typeof setInterval> | null = null;
let sidecarHealthInitialTimer: ReturnType<typeof setTimeout> | null = null;

function startSidecarHealthPolling(): void {
  if (sidecarHealthTimer) return;
  sidecarHealthInitialTimer = setTimeout(checkSidecarHealth, 5000);
  sidecarHealthTimer = setInterval(checkSidecarHealth, 30000);
}

function stopSidecarHealthPolling(): void {
  if (sidecarHealthInitialTimer) {
    clearTimeout(sidecarHealthInitialTimer);
    sidecarHealthInitialTimer = null;
  }
  if (sidecarHealthTimer) {
    clearInterval(sidecarHealthTimer);
    sidecarHealthTimer = null;
  }
}

// --- WebSocket Connection (EXT-06) ---

let ws: WebSocket | null = null;
let wsState: WsState = "disconnected";
let wsReconnectDelay = 1000;
let wsReconnectTimer: ReturnType<typeof setTimeout> | null = null;
let wsSubscriptionId = 1;

function setWsState(state: WsState): void {
  wsState = state;
  browser.runtime.sendMessage({ type: "WS_STATE_CHANGED", state }).catch(() => {});
}

async function getUserId(): Promise<string | null> {
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    try {
      const payloadRaw = JSON.parse(atob(tokens.access_token.split(".")[1] || ""));
      const payload = JwtSubPayloadSchema.safeParse(payloadRaw);
      if (payload.success && payload.data.sub) return payload.data.sub;
    } catch { /* fall through */ }
  }
  return null;
}

async function connectWebSocket(): Promise<void> {
  if (ws) {
    ws.onclose = null;
    ws.onerror = null;
    ws.onmessage = null;
    ws.close();
    ws = null;
  }

  if (wsReconnectTimer) {
    clearTimeout(wsReconnectTimer);
    wsReconnectTimer = null;
  }

  const settings = await getSettings();
  if (!settings.wsUrl) {
    setWsState("disconnected");
    return;
  }

  setWsState("connecting");

  try {
    ws = new WebSocket(settings.wsUrl);
  } catch {
    setWsState("disconnected");
    scheduleReconnect();
    return;
  }

  ws.onopen = async () => {
    console.log("WS connected to", settings.wsUrl);
    wsReconnectDelay = WS_BASE_RECONNECT_DELAY;
    setWsState("connected");

    const userId = await getUserId();
    if (userId) {
      const subMsg = {
        method: "SUBSCRIBE",
        params: [`order.${userId}`],
        id: wsSubscriptionId++,
      };
      ws?.send(JSON.stringify(subMsg));
      console.log("WS subscribed to order." + userId);
    }
  };

  ws.onmessage = (event: MessageEvent) => {
    try {
      const msgRaw = typeof event.data === "string" ? JSON.parse(event.data) : event.data;
      const msg = WebSocketMessageSchema.safeParse(msgRaw);
      if (!msg.success) {
        return;
      }

      const wsMsg = msg.data;
      if (wsMsg.stream && wsMsg.stream.startsWith("order.")) {
        forwardOrderUpdate(wsMsg.data);
      }
      if (wsMsg.stream === "sidecar.health") {
        const data = SidecarStreamDataSchema.safeParse(wsMsg.data);
        setSidecarStatus(data.success && data.data.status === "healthy" ? "healthy" : "unreachable");
      }
    } catch {
      console.warn("WS: failed to parse message", event.data);
    }
  };

  ws.onclose = () => {
    console.log("WS disconnected");
    ws = null;
    setWsState("disconnected");
    scheduleReconnect();
  };

  ws.onerror = (event: Event) => {
    console.warn("WS error", event);
  };
}

function scheduleReconnect(): void {
  if (wsReconnectTimer) return;
  wsReconnectTimer = setTimeout(() => {
    wsReconnectTimer = null;
    wsReconnectDelay = nextReconnectDelay(wsReconnectDelay);
    connectWebSocket();
  }, wsReconnectDelay);
}

function disconnectWebSocket(): void {
  if (wsReconnectTimer) {
    clearTimeout(wsReconnectTimer);
    wsReconnectTimer = null;
  }
  if (ws) {
    ws.onclose = null;
    ws.close();
    ws = null;
  }
  setWsState("disconnected");
}

// Cached content tabs — invalidated on tab create/remove
let cachedContentTabs: browser.Tabs.Tab[] | null = null;
const CONTENT_TAB_URLS = ["*://*.tradingview.com/*", "*://*.dexscreener.com/*", "*://*.gmx.io/*", "*://*.bybit.com/*"];

browser.tabs.onCreated.addListener(() => { cachedContentTabs = null; });
browser.tabs.onRemoved.addListener(() => { cachedContentTabs = null; });

async function getContentTabs(): Promise<browser.Tabs.Tab[]> {
  if (!cachedContentTabs) {
    cachedContentTabs = await browser.tabs.query({ url: CONTENT_TAB_URLS });
  }
  return cachedContentTabs;
}

function forwardOrderUpdate(data: unknown): void {
  getContentTabs().then((tabs) => {
    for (const tab of tabs) {
      if (tab.id) {
        browser.tabs.sendMessage(tab.id, {
          type: "WS_ORDER_UPDATE",
          data,
        }).catch(() => {});
      }
    }
  });

  browser.runtime.sendMessage({ type: "WS_ORDER_UPDATE", data }).catch(() => {});
}

// --- Message Router ---

browser.runtime.onMessage.addListener((message: unknown) => {
  const parsed = RuntimeMessageSchema.safeParse(message);
  if (!parsed.success) {
    return undefined;
  }
  const msg = parsed.data;

  if (msg.type === "GET_SETTINGS") {
    return getSettings();
  }

  if (msg.type === "EXECUTE_TRADE") {
    return executeTrade(msg.payload);
  }

  if (msg.type === "LOGIN") {
    return login(msg.email, msg.password).then(async (result) => {
      if (result.success) await ensureActiveExchange();
      return result;
    });
  }

  if (msg.type === "LOGOUT") {
    if (refreshTimer) {
      clearTimeout(refreshTimer);
      refreshTimer = null;
    }
    disconnectWebSocket();
    stopSidecarHealthPolling();
    return clearTokens().then(() => ({ success: true }));
  }

  if (msg.type === "AUTH_STATUS") {
    return getAuthStatus();
  }

  if (msg.type === "REFRESH_TOKEN") {
    return refreshAccessToken().then((ok) => ({ success: ok }));
  }

  if (msg.type === "WS_STATUS") {
    // Auto-reconnect if disconnected when popup queries status
    if (wsState === "disconnected" && !wsReconnectTimer) {
      wsReconnectDelay = WS_BASE_RECONNECT_DELAY;
      connectWebSocket();
    }
    return Promise.resolve({ state: wsState });
  }

  if (msg.type === "WS_RECONNECT") {
    connectWebSocket();
    return Promise.resolve({ success: true });
  }

  if (msg.type === "LIST_TRADES") {
    return listTrades();
  }

  if (msg.type === "CANCEL_TRADE") {
    return cancelTrade(msg.tradeId);
  }

  if (msg.type === "CLEANUP_TRADES") {
    return cleanupTrades();
  }

  // EXT-19: GET_BALANCE always fetches live balance from active exchange
  if (msg.type === "GET_BALANCE") {
    return getLiveBalance();
  }

  if (msg.type === "REGISTER") {
    return register(msg.email, msg.password).then(async (result) => {
      if (result.success) await ensureActiveExchange();
      return result;
    });
  }

  if (msg.type === "LIST_EXCHANGES") {
    return listExchanges();
  }

  if (msg.type === "LIST_EXCHANGE_ACCOUNTS") {
    return listExchangeAccounts();
  }

  if (msg.type === "ADD_EXCHANGE_ACCOUNT") {
    return addExchangeAccount(msg.payload).then((result) => {
      if (result.success) ensureActiveExchange();
      return result;
    });
  }

  if (msg.type === "DELETE_EXCHANGE_ACCOUNT") {
    return deleteExchangeAccount(msg.accountId).then(async (result) => {
      if (result.success) await ensureActiveExchange();
      return result;
    });
  }

  if (msg.type === "TEST_EXCHANGE_CONNECTION") {
    return testExchangeConnection(msg.accountId);
  }

  if (msg.type === "GET_ACTIVE_EXCHANGE") {
    return getActiveExchangeId().then((id) => ({ exchangeId: id }));
  }

  if (msg.type === "SET_ACTIVE_EXCHANGE") {
    return setActiveExchangeId(msg.exchangeId).then(() => ({ success: true }));
  }

  if (msg.type === "TOKEN_SYNCED_FROM_WEB") {
    return getTokens().then((tokens) => {
      if (tokens && tokens.expires_in > 0) {
        scheduleTokenRefresh(tokens.expires_in);
        ensureActiveExchange();
        debouncedConnectWebSocket();
      }
      return { success: true };
    });
  }

  if (msg.type === "SIDECAR_STATUS") {
    return Promise.resolve({ status: sidecarStatus });
  }

  if (msg.type === "EXCHANGE_POSITIONS") {
    return fetchExchangePositions();
  }

  if (msg.type === "CLOSE_EXCHANGE_POSITION") {
    return closeExchangePosition(msg.symbol, msg.side, msg.contracts);
  }

  if (msg.type === "FORGOT_PASSWORD") {
    return forgotPassword(msg.email);
  }

  if (msg.type === "GET_EXCHANGE_MODE") {
    return getExchangeMode().then((mode) => ({ mode }));
  }

  if (msg.type === "SET_EXCHANGE_MODE") {
    return browser.storage.local.set({ exchangeMode: msg.mode }).then(async () => {
      await ensureActiveExchange();
      return { success: true };
    });
  }

  // EXT-33: Handle wallet connection event from web app via token-sync content script
  if (msg.type === "ACCOUNT_LINKED") {
    return (async () => {
      // Small delay to allow backend to persist the account
      await new Promise((r) => setTimeout(r, 500));
      const result = await listExchangeAccounts();
      if (result.success && result.data) {
        await browser.storage.local.set({ exchangeAccounts: result.data });
      }
      await ensureActiveExchange();
      return { success: true };
    })();
  }
});

// On startup, migrate legacy exchange ID, schedule token refresh, then connect WebSocket
migrateActiveExchangeId().then(() => {
  getTokens().then((tokens) => {
    if (tokens && tokens.expires_in > 0) {
      scheduleTokenRefresh(tokens.expires_in);
      ensureActiveExchange();
    }
  });
});

// EXT-06: Connect WebSocket on startup
connectWebSocket();

// EXT-16: Start sidecar health polling
startSidecarHealthPolling();

// Reconnect WebSocket when settings change (debounced to collapse rapid changes)
let wsDebounceTimer: ReturnType<typeof setTimeout> | null = null;

function debouncedConnectWebSocket(): void {
  if (wsDebounceTimer) clearTimeout(wsDebounceTimer);
  wsDebounceTimer = setTimeout(() => {
    wsDebounceTimer = null;
    connectWebSocket();
  }, 300);
}

browser.storage.onChanged.addListener((changes) => {
  if (changes.wsUrl) {
    debouncedConnectWebSocket();
  }
});

// Export for testing — unused at runtime, tree-shaken by esbuild
export { disconnectWebSocket as _disconnectWebSocket };
