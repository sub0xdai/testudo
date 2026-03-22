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

// --- Shared API Request Helper ---

type AuthMode = "hard" | "soft" | "none";

interface ApiOpts {
  method?: string;
  body?: unknown;
  auth?: AuthMode;
  authError?: string;
  timeout?: number;
}

type ApiResult = { ok: true; raw: unknown } | { ok: false; error: string; httpError?: boolean };

async function apiRequest(
  endpoint: string,
  opts: ApiOpts = {},
  retried = false,
): Promise<ApiResult> {
  const { method, body, auth = "none", authError, timeout } = opts;
  const settings = await getSettings();
  const tokens = await getTokens();

  if (auth === "hard" && (!tokens || tokens.expires_in <= 0)) {
    return { ok: false, error: authError || "Authentication required" };
  }

  const headers: Record<string, string> = {};
  if (auth === "hard" || (auth === "soft" && tokens && tokens.expires_in > 0)) {
    headers["Authorization"] = `Bearer ${tokens!.access_token}`;
  }
  if (body !== undefined) headers["Content-Type"] = "application/json";

  const init: RequestInit = { method, headers };
  if (body !== undefined) init.body = JSON.stringify(body);
  if (timeout) init.signal = AbortSignal.timeout(timeout);

  try {
    const response = await fetch(`${settings.backendUrl}${endpoint}`, init);

    if (!response.ok) {
      if (response.status === 401 && !retried && auth !== "none") {
        const canRetry = auth === "hard" || (tokens && tokens.expires_in > 0);
        if (canRetry) {
          const refreshed = await refreshAccessToken();
          if (refreshed) return apiRequest(endpoint, opts, true);
        }
      }
      const raw = await response.json().catch(() => ({}));
      const json = ErrorResponseSchema.safeParse(raw);
      const errorMsg = json.success ? (json.data.error || json.data.message) : undefined;
      return { ok: false, error: errorMsg || `HTTP ${response.status}`, httpError: true };
    }

    const raw = await response.json().catch(() => ({}));
    return { ok: true, raw };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { ok: false, error: msg };
  }
}

async function authenticate(endpoint: string, email: string, password: string): Promise<{ success: boolean; error?: string }> {
  const result = await apiRequest(endpoint, { method: "POST", body: { email, password } });
  if (!result.ok) return { success: false, error: result.error };
  const parsed = LoginResponseSchema.safeParse(result.raw);
  if (!parsed.success) return { success: false, error: "Unexpected server response" };
  await storeTokens(parsed.data.tokens);
  scheduleTokenRefresh(parsed.data.tokens.expires_in);
  return { success: true };
}

function login(email: string, password: string) {
  return authenticate("/api/v1/auth/login", email, password);
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

  const result = await apiRequest("/api/v1/auth/refresh", {
    method: "POST",
    body: { refresh_token: tokens.refresh_token },
  });

  if (!result.ok) {
    if (result.httpError) await clearTokens();
    return false;
  }

  const parsed = RefreshResponseSchema.safeParse(result.raw);
  if (!parsed.success) return false;
  await storeTokens(parsed.data.tokens);
  scheduleTokenRefresh(parsed.data.tokens.expires_in);
  return true;
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

async function executeTrade(payload: RuntimeTradePayload): Promise<BackendResponse> {
  if (tradeInFlight) return { success: false, error: "Trade already in progress" };
  tradeInFlight = true;

  try {
    const activeExchangeId = payload.exchange_account_id || await getActiveExchangeId();
    const body: Record<string, unknown> = {
      symbol: normalizeSymbol(payload.symbol),
      side: mapSide(payload.side),
      entry_price: payload.entry.toString(),
      stop_loss_price: payload.stop.toString(),
      take_profit_price: payload.target.toString(),
      management: payload.management,
    };
    if (activeExchangeId) body.exchange_account_id = activeExchangeId;

    const result = await apiRequest("/api/v1/trades", {
      method: "POST", body, auth: "hard",
      authError: "Authentication required — please log in",
    });
    if (!result.ok) return { success: false, error: result.error };

    const normalized = normalizeBackendAck(result.raw);
    const validated = BackendResponseSchema.safeParse(normalized);
    if (!validated.success) return { success: false, error: "Malformed trade response" };
    return validated.data;
  } finally {
    tradeInFlight = false;
  }
}

// --- Trade Listing (Active Orders) ---

async function listTrades(): Promise<{ success: boolean; data?: TradeGroupResponse[]; error?: string }> {
  const result = await apiRequest("/api/v1/trades", { auth: "hard" });
  if (!result.ok) return { success: false, error: result.error };

  const normalized = normalizeTradeListResponse(result.raw);
  const validated = TradeListResponseSchema.safeParse(normalized);
  if (!validated.success) return { success: false, error: "Malformed trade list response" };
  if (!validated.data.success) return { success: false, error: validated.data.error || "Trade list request failed" };
  return { success: true, data: validated.data.data || [] };
}

async function cancelTrade(tradeId: string): Promise<BackendResponse> {
  const result = await apiRequest(`/api/v1/trades/${tradeId}`, {
    method: "DELETE", auth: "hard",
  });
  if (!result.ok) return { success: false, error: result.error };

  const normalized = normalizeBackendAck(result.raw);
  const validated = BackendResponseSchema.safeParse(normalized);
  if (!validated.success) return { success: false, error: "Malformed cancel response" };
  return validated.data;
}

async function cleanupTrades(): Promise<BackendResponse> {
  const result = await apiRequest("/api/v1/trades/cleanup", {
    method: "POST", auth: "hard",
  });
  if (!result.ok) return { success: false, error: result.error };
  return { success: true };
}

// --- Registration (EXT-15 FR-2) ---

function register(email: string, password: string) {
  return authenticate("/api/v1/auth/register", email, password);
}

// --- Password Reset (AUD-08 FR-5) ---

async function forgotPassword(email: string): Promise<{ success: boolean; error?: string }> {
  const result = await apiRequest("/api/v1/auth/forgot-password", {
    method: "POST", body: { email },
  });
  if (!result.ok) return { success: false, error: result.error };
  return { success: true };
}

// --- Exchange Account Management (EXT-15 FR-4) ---

async function listExchanges(): Promise<{ success: boolean; data?: ExchangeInfo[]; error?: string }> {
  const result = await apiRequest("/api/v1/exchanges", { auth: "soft" });
  if (!result.ok) return { success: false, error: result.error };

  const json = ListExchangesResponseSchema.safeParse(result.raw);
  if (!json.success) return { success: false, error: "Malformed exchanges response" };
  return { success: true, data: json.data.exchanges || [] };
}

async function listExchangeAccounts(): Promise<{ success: boolean; data?: ExchangeAccount[]; error?: string }> {
  const result = await apiRequest("/api/v1/exchanges/accounts", { auth: "soft" });
  if (!result.ok) return { success: false, error: result.error };

  console.log("[listExchangeAccounts] raw response:", JSON.stringify(result.raw).slice(0, 500));
  const json = ExchangeAccountsResponseSchema.safeParse(result.raw);
  if (!json.success) {
    console.error("[listExchangeAccounts] schema parse failed:", json.error.issues);
    return { success: false, error: "Malformed exchange accounts response" };
  }
  const accounts = Array.isArray(json.data) ? json.data : (json.data.data || json.data.accounts || []);
  console.log("[listExchangeAccounts] parsed accounts:", accounts.length, accounts.map(a => a.exchange_name));
  return { success: true, data: accounts };
}

async function addExchangeAccount(payload: AddExchangeAccountPayload): Promise<{ success: boolean; data?: ExchangeAccount; error?: string }> {
  const result = await apiRequest("/api/v1/exchanges/accounts", {
    method: "POST", body: payload, auth: "soft",
  });
  if (!result.ok) return { success: false, error: result.error };

  const json = AddExchangeAccountResponseSchema.safeParse(result.raw);
  if (!json.success) return { success: false, error: "Malformed add account response" };
  return { success: true, data: json.data.data };
}

async function deleteExchangeAccount(accountId: string): Promise<{ success: boolean; error?: string }> {
  const result = await apiRequest(`/api/v1/exchanges/accounts/${accountId}`, {
    method: "DELETE", auth: "soft",
  });
  if (!result.ok) return { success: false, error: result.error };
  return { success: true };
}

async function testExchangeConnection(accountId: string): Promise<{ success: boolean; data?: TestConnectionResult; error?: string }> {
  const result = await apiRequest(`/api/v1/exchanges/accounts/${accountId}/test`, {
    method: "POST", auth: "soft",
  });
  if (!result.ok) return { success: false, error: result.error };

  const json = TestConnectionResultSchema.safeParse(result.raw);
  if (!json.success) return { success: false, error: "Malformed connection test response" };
  return { success: true, data: json.data };
}

// --- EXT-19: Live Exchange Balance (always from active exchange) ---

async function getLiveBalance(): Promise<{ success: boolean; data?: LiveBalanceResponse; error?: string }> {
  let activeId = await getActiveExchangeId();
  if (!activeId) {
    activeId = await ensureActiveExchange();
    if (!activeId) return { success: false, error: "No active exchange selected" };
  }

  const result = await apiRequest(`/api/v1/exchanges/accounts/${activeId}/balance`, {
    auth: "hard", authError: "Authentication required for live balance", timeout: 10000,
  });
  if (!result.ok) return { success: false, error: result.error };

  const json = ExchangeBalanceApiResponseSchema.safeParse(result.raw);
  if (!json.success) return { success: false, error: "Malformed balance response" };

  const balances: BalanceResponse[] = json.data.balances.map((b) => ({
    asset: b.asset, available: b.free, locked: b.used,
  }));
  return { success: true, data: { exchange_name: json.data.exchange_name, balances } };
}

// --- Exchange Positions (live fallback) ---

async function fetchExchangePositions(): Promise<{ success: boolean; data?: ExchangePositionsResponse; error?: string }> {
  let activeId = await getActiveExchangeId();
  if (!activeId) {
    activeId = await ensureActiveExchange();
    if (!activeId) return { success: false, error: "No active exchange selected" };
  }

  const result = await apiRequest(`/api/v1/exchanges/accounts/${activeId}/positions`, {
    auth: "hard", timeout: 15000,
  });
  if (!result.ok) return { success: false, error: result.error };

  const json = ExchangePositionsApiResponseSchema.safeParse(result.raw);
  if (!json.success) return { success: false, error: "Malformed positions response" };
  return { success: true, data: json.data };
}

// --- EXT-34: Close Exchange Position ---

async function closeExchangePosition(
  symbol: string,
  side: string,
  contracts: string,
): Promise<{ success: boolean; error?: string }> {
  let activeId = await getActiveExchangeId();
  if (!activeId) {
    activeId = await ensureActiveExchange();
    if (!activeId) return { success: false, error: "No active exchange selected" };
  }

  const result = await apiRequest(`/api/v1/exchanges/accounts/${activeId}/close-position`, {
    method: "POST", body: { symbol, side, contracts }, auth: "hard", timeout: 30000,
  });
  if (!result.ok) return { success: false, error: result.error };
  return { success: true };
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
  const result = await apiRequest("/api/v1/health/sidecar", { timeout: 5000 });
  if (!result.ok) { setSidecarStatus("unreachable"); return; }
  const json = SidecarHealthResponseSchema.safeParse(result.raw);
  setSidecarStatus(json.success && json.data.status === "healthy" ? "healthy" : "unreachable");
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

browser.tabs.onCreated?.addListener(() => { cachedContentTabs = null; });
browser.tabs.onRemoved?.addListener(() => { cachedContentTabs = null; });

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

// --- Message Handlers ---

type ParsedMessage = ReturnType<typeof RuntimeMessageSchema.parse>;
type MessageHandler = (msg: ParsedMessage) => Promise<unknown> | unknown;
type MsgOf<T extends ParsedMessage["type"]> = Extract<ParsedMessage, { type: T }>;

function handleGetSettings(): Promise<unknown> {
  return getSettings();
}

function handleExecuteTrade(msg: ParsedMessage): Promise<unknown> {
  return executeTrade((msg as MsgOf<"EXECUTE_TRADE">).payload);
}

function handleLogin(msg: ParsedMessage): Promise<unknown> {
  const { email, password } = msg as MsgOf<"LOGIN">;
  return login(email, password).then(async (result) => {
    if (result.success) await ensureActiveExchange();
    return result;
  });
}

function handleRegister(msg: ParsedMessage): Promise<unknown> {
  const { email, password } = msg as MsgOf<"REGISTER">;
  return register(email, password).then(async (result) => {
    if (result.success) await ensureActiveExchange();
    return result;
  });
}

function handleLogout(): Promise<unknown> {
  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
  }
  disconnectWebSocket();
  stopSidecarHealthPolling();
  return clearTokens().then(() => ({ success: true }));
}

function handleAuthStatus(): Promise<unknown> {
  return getAuthStatus();
}

function handleRefreshToken(): Promise<unknown> {
  return refreshAccessToken().then((ok) => ({ success: ok }));
}

function handleWsStatus(): Promise<unknown> {
  // Auto-reconnect if disconnected when popup queries status
  if (wsState === "disconnected" && !wsReconnectTimer) {
    wsReconnectDelay = WS_BASE_RECONNECT_DELAY;
    connectWebSocket();
  }
  return Promise.resolve({ state: wsState });
}

function handleWsReconnect(): Promise<unknown> {
  connectWebSocket();
  return Promise.resolve({ success: true });
}

function handleListTrades(): Promise<unknown> {
  return listTrades();
}

function handleCancelTrade(msg: ParsedMessage): Promise<unknown> {
  return cancelTrade((msg as MsgOf<"CANCEL_TRADE">).tradeId);
}

function handleCleanupTrades(): Promise<unknown> {
  return cleanupTrades();
}

function handleGetBalance(): Promise<unknown> {
  return getLiveBalance();
}

function handleListExchanges(): Promise<unknown> {
  return listExchanges();
}

function handleListExchangeAccounts(): Promise<unknown> {
  return listExchangeAccounts();
}

function handleAddExchangeAccount(msg: ParsedMessage): Promise<unknown> {
  return addExchangeAccount((msg as MsgOf<"ADD_EXCHANGE_ACCOUNT">).payload).then((result) => {
    if (result.success) ensureActiveExchange();
    return result;
  });
}

function handleDeleteExchangeAccount(msg: ParsedMessage): Promise<unknown> {
  return deleteExchangeAccount((msg as MsgOf<"DELETE_EXCHANGE_ACCOUNT">).accountId).then(async (result) => {
    if (result.success) await ensureActiveExchange();
    return result;
  });
}

function handleTestExchangeConnection(msg: ParsedMessage): Promise<unknown> {
  return testExchangeConnection((msg as MsgOf<"TEST_EXCHANGE_CONNECTION">).accountId);
}

function handleGetActiveExchange(): Promise<unknown> {
  return getActiveExchangeId().then((id) => ({ exchangeId: id }));
}

function handleSetActiveExchange(msg: ParsedMessage): Promise<unknown> {
  return setActiveExchangeId((msg as MsgOf<"SET_ACTIVE_EXCHANGE">).exchangeId).then(() => ({ success: true }));
}

function handleTokenSyncedFromWeb(): Promise<unknown> {
  return getTokens().then((tokens) => {
    if (tokens && tokens.expires_in > 0) {
      scheduleTokenRefresh(tokens.expires_in);
      ensureActiveExchange();
      debouncedConnectWebSocket();
    }
    return { success: true };
  });
}

function handleSidecarStatus(): Promise<unknown> {
  return Promise.resolve({ status: sidecarStatus });
}

function handleExchangePositions(): Promise<unknown> {
  return fetchExchangePositions();
}

function handleCloseExchangePosition(msg: ParsedMessage): Promise<unknown> {
  const { symbol, side, contracts } = msg as MsgOf<"CLOSE_EXCHANGE_POSITION">;
  return closeExchangePosition(symbol, side, contracts);
}

function handleForgotPassword(msg: ParsedMessage): Promise<unknown> {
  return forgotPassword((msg as MsgOf<"FORGOT_PASSWORD">).email);
}

function handleGetExchangeMode(): Promise<unknown> {
  return getExchangeMode().then((mode) => ({ mode }));
}

function handleSetExchangeMode(msg: ParsedMessage): Promise<unknown> {
  return browser.storage.local.set({ exchangeMode: (msg as MsgOf<"SET_EXCHANGE_MODE">).mode }).then(async () => {
    await ensureActiveExchange();
    return { success: true };
  });
}

function handleAccountLinked(): Promise<unknown> {
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

// --- Message Dispatch ---

const messageHandlers: Record<string, MessageHandler> = {
  GET_SETTINGS: handleGetSettings,
  EXECUTE_TRADE: handleExecuteTrade,
  LOGIN: handleLogin,
  REGISTER: handleRegister,
  LOGOUT: handleLogout,
  AUTH_STATUS: handleAuthStatus,
  REFRESH_TOKEN: handleRefreshToken,
  WS_STATUS: handleWsStatus,
  WS_RECONNECT: handleWsReconnect,
  LIST_TRADES: handleListTrades,
  CANCEL_TRADE: handleCancelTrade,
  CLEANUP_TRADES: handleCleanupTrades,
  GET_BALANCE: handleGetBalance,
  LIST_EXCHANGES: handleListExchanges,
  LIST_EXCHANGE_ACCOUNTS: handleListExchangeAccounts,
  ADD_EXCHANGE_ACCOUNT: handleAddExchangeAccount,
  DELETE_EXCHANGE_ACCOUNT: handleDeleteExchangeAccount,
  TEST_EXCHANGE_CONNECTION: handleTestExchangeConnection,
  GET_ACTIVE_EXCHANGE: handleGetActiveExchange,
  SET_ACTIVE_EXCHANGE: handleSetActiveExchange,
  TOKEN_SYNCED_FROM_WEB: handleTokenSyncedFromWeb,
  SIDECAR_STATUS: handleSidecarStatus,
  EXCHANGE_POSITIONS: handleExchangePositions,
  CLOSE_EXCHANGE_POSITION: handleCloseExchangePosition,
  FORGOT_PASSWORD: handleForgotPassword,
  GET_EXCHANGE_MODE: handleGetExchangeMode,
  SET_EXCHANGE_MODE: handleSetExchangeMode,
  ACCOUNT_LINKED: handleAccountLinked,
};

browser.runtime.onMessage.addListener((message: unknown) => {
  const parsed = RuntimeMessageSchema.safeParse(message);
  if (!parsed.success) return undefined;
  const handler = messageHandlers[parsed.data.type];
  return handler ? handler(parsed.data) : undefined;
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
