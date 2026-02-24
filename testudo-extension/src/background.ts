import browser from "webextension-polyfill";
import type { Settings, AuthTokens, LoginResponse, TradePayload, BackendResponse, WsState, TradeGroupResponse, BalanceResponse, ExchangeInfo, ExchangeAccount, AddExchangeAccountPayload, TestConnectionResult } from "./types";
import {
  DEFAULT_SETTINGS, PAPER_USER_ID, WS_BASE_RECONNECT_DELAY,
  normalizeSymbol, mapSide, calculateRefreshDelay, nextReconnectDelay,
} from "./utils";

// Background service worker — manages settings, auth, REST dispatch, and WebSocket connection.

async function getSettings(): Promise<Settings> {
  const stored = await browser.storage.local.get(["backendUrl", "wsUrl", "executionMode"]);
  return {
    backendUrl: (stored.backendUrl as string) || DEFAULT_SETTINGS.backendUrl,
    wsUrl: (stored.wsUrl as string) || DEFAULT_SETTINGS.wsUrl,
    executionMode: (stored.executionMode as Settings["executionMode"]) || DEFAULT_SETTINGS.executionMode,
  };
}

browser.runtime.onInstalled.addListener(async () => {
  const settings = await getSettings();
  await browser.storage.local.set({ ...settings });
  console.log("Testudo Sniper installed", settings);
});

// --- Auth Token Management (EXT-05 FR-2, FR-3, FR-7) ---

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
    const payload = JSON.parse(atob(tokens.access_token.split(".")[1])) as { email?: string };
    return { authenticated: true, email: payload.email };
  } catch {
    return { authenticated: true };
  }
}

// --- Active Exchange Selection (EXT-16 FR-3) ---

async function getActiveExchangeId(): Promise<string | null> {
  const stored = await browser.storage.local.get(["activeExchangeId"]);
  return (stored.activeExchangeId as string) || null;
}

async function setActiveExchangeId(id: string | null): Promise<void> {
  if (id) {
    await browser.storage.local.set({ activeExchangeId: id });
  } else {
    await browser.storage.local.remove(["activeExchangeId"]);
  }
}

// --- Trade Execution (EXT-08: management block, no client-side quantity) ---

async function executeTrade(payload: TradePayload, retried = false): Promise<BackendResponse> {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/trades`;

  // EXT-16 FR-3.5: Attach active exchange account ID to trade payload
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

  const headers: Record<string, string> = { "Content-Type": "application/json" };
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  } else {
    headers["X-User-Id"] = PAPER_USER_ID;
  }

  headers["X-Execution-Mode"] = settings.executionMode;

  try {
    const response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
    });

    const json = await response.json() as BackendResponse;

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) {
          return executeTrade(payload, true);
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

// --- Trade Listing (Active Orders) ---

async function listTrades(retried = false): Promise<{ success: boolean; data?: TradeGroupResponse[]; error?: string }> {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/trades`;

  const headers: Record<string, string> = {};
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  } else {
    headers["X-User-Id"] = PAPER_USER_ID;
  }

  try {
    const response = await fetch(url, { headers });
    const json = await response.json() as { success: boolean; data?: TradeGroupResponse[]; error?: string };

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return listTrades(true);
      }
      return { success: false, error: json.error || `HTTP ${response.status}` };
    }

    return json;
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

async function getBalances(retried = false): Promise<{ success: boolean; data?: BalanceResponse[]; error?: string }> {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/paper/balances`;

  const headers: Record<string, string> = {};
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  } else {
    headers["X-User-Id"] = PAPER_USER_ID;
  }

  try {
    const response = await fetch(url, { headers });
    const json = await response.json() as { success: boolean; data?: BalanceResponse[]; error?: string };

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return getBalances(true);
      }
      return { success: false, error: json.error || `HTTP ${response.status}` };
    }

    return json;
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { success: false, error: msg };
  }
}

async function cancelTrade(tradeId: string): Promise<BackendResponse> {
  const settings = await getSettings();
  const url = `${settings.backendUrl}/api/v1/trades/${tradeId}`;

  const headers: Record<string, string> = {};
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    headers["Authorization"] = `Bearer ${tokens.access_token}`;
  } else {
    headers["X-User-Id"] = PAPER_USER_ID;
  }

  try {
    const response = await fetch(url, { method: "DELETE", headers });
    const json = await response.json() as BackendResponse;

    if (!response.ok) {
      return { success: false, error: json.error || `HTTP ${response.status}` };
    }

    return json;
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
      const json = await response.json() as { error?: string; message?: string };
      return { success: false, error: json.message || json.error || `HTTP ${response.status}` };
    }

    const json = await response.json() as LoginResponse;
    await storeTokens(json.tokens);
    scheduleTokenRefresh(json.tokens.expires_in);
    return { success: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Registration failed";
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
    const json = await response.json() as { exchanges?: ExchangeInfo[]; error?: string };

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return listExchanges(true);
      }
      return { success: false, error: json.error || `HTTP ${response.status}` };
    }

    return { success: true, data: json.exchanges || [] };
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
    const json = await response.json() as { success?: boolean; data?: ExchangeAccount[]; accounts?: ExchangeAccount[]; error?: string };

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return listExchangeAccounts(true);
      }
      return { success: false, error: json.error || `HTTP ${response.status}` };
    }

    return { success: true, data: json.data || json.accounts || [] };
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
    const json = await response.json() as { success?: boolean; data?: ExchangeAccount; error?: string };

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return addExchangeAccount(payload, true);
      }
      return { success: false, error: json.error || `HTTP ${response.status}` };
    }

    return { success: true, data: json.data };
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
      const json = await response.json().catch(() => ({})) as { error?: string };
      return { success: false, error: json.error || `HTTP ${response.status}` };
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
    const json = await response.json() as TestConnectionResult & { error?: string };

    if (!response.ok) {
      if (response.status === 401 && tokens && !retried) {
        const refreshed = await refreshAccessToken();
        if (refreshed) return testExchangeConnection(accountId, true);
      }
      return { success: false, error: json.error || `HTTP ${response.status}` };
    }

    return { success: true, data: json };
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
      const json = await response.json() as { status?: string };
      setSidecarStatus(json.status === "healthy" ? "healthy" : "unreachable");
    } else {
      setSidecarStatus("unreachable");
    }
  } catch {
    setSidecarStatus("unreachable");
  }
}

// Poll sidecar health every 30 seconds
let sidecarHealthTimer: ReturnType<typeof setInterval> | null = null;

function startSidecarHealthPolling(): void {
  if (sidecarHealthTimer) return;
  // Defer first check to avoid interfering with concurrent operations at startup
  setTimeout(checkSidecarHealth, 5000);
  sidecarHealthTimer = setInterval(checkSidecarHealth, 30000);
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

async function getUserId(): Promise<string> {
  const tokens = await getTokens();
  if (tokens && tokens.expires_in > 0) {
    try {
      const payload = JSON.parse(atob(tokens.access_token.split(".")[1])) as { sub?: string };
      if (payload.sub) return payload.sub;
    } catch { /* fall through */ }
  }
  return PAPER_USER_ID;
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
    const subMsg = {
      method: "SUBSCRIBE",
      params: [`order.${userId}`],
      id: wsSubscriptionId++,
    };
    ws?.send(JSON.stringify(subMsg));
    console.log("WS subscribed to order." + userId);
  };

  ws.onmessage = (event: MessageEvent) => {
    try {
      const msg = JSON.parse(event.data as string) as { stream?: string; data?: unknown };
      if (msg.stream && msg.stream.startsWith("order.")) {
        forwardOrderUpdate(msg.data);
      }
      // EXT-16 FR-2.2: Listen for sidecar health events
      if (msg.stream === "sidecar.health") {
        const data = msg.data as { status?: string };
        setSidecarStatus(data?.status === "healthy" ? "healthy" : "unreachable");
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

function forwardOrderUpdate(data: unknown): void {
  // Forward to all content script tabs
  browser.tabs.query({ url: ["*://*.tradingview.com/*", "*://*.dexscreener.com/*", "*://*.gmx.io/*", "*://*.bybit.com/*"] }).then((tabs) => {
    for (const tab of tabs) {
      if (tab.id) {
        browser.tabs.sendMessage(tab.id, {
          type: "WS_ORDER_UPDATE",
          data,
        }).catch(() => {});
      }
    }
  });

  // Broadcast to extension pages (popup) for real-time order updates
  browser.runtime.sendMessage({ type: "WS_ORDER_UPDATE", data }).catch(() => {});
}

// --- Message Router ---

type Message =
  | { type: "GET_SETTINGS" }
  | { type: "EXECUTE_TRADE"; payload: TradePayload }
  | { type: "LOGIN"; email: string; password: string }
  | { type: "REGISTER"; email: string; password: string }
  | { type: "LOGOUT" }
  | { type: "AUTH_STATUS" }
  | { type: "REFRESH_TOKEN" }
  | { type: "WS_STATUS" }
  | { type: "WS_RECONNECT" }
  | { type: "LIST_TRADES" }
  | { type: "CANCEL_TRADE"; tradeId: string }
  | { type: "GET_BALANCES" }
  | { type: "LIST_EXCHANGES" }
  | { type: "LIST_EXCHANGE_ACCOUNTS" }
  | { type: "ADD_EXCHANGE_ACCOUNT"; payload: AddExchangeAccountPayload }
  | { type: "DELETE_EXCHANGE_ACCOUNT"; accountId: string }
  | { type: "TEST_EXCHANGE_CONNECTION"; accountId: string }
  | { type: "GET_ACTIVE_EXCHANGE" }
  | { type: "SET_ACTIVE_EXCHANGE"; exchangeId: string | null }
  | { type: "SIDECAR_STATUS" };

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

  if (msg.type === "WS_STATUS") {
    return Promise.resolve({ state: wsState });
  }

  if (msg.type === "WS_RECONNECT") {
    connectWebSocket();
    return Promise.resolve({ success: true });
  }

  if (msg.type === "LIST_TRADES") {
    return listTrades();
  }

  if (msg.type === "CANCEL_TRADE" && "tradeId" in msg) {
    return cancelTrade(msg.tradeId);
  }

  if (msg.type === "GET_BALANCES") {
    return getBalances();
  }

  if (msg.type === "REGISTER" && "email" in msg && "password" in msg) {
    return register(msg.email, msg.password);
  }

  if (msg.type === "LIST_EXCHANGES") {
    return listExchanges();
  }

  if (msg.type === "LIST_EXCHANGE_ACCOUNTS") {
    return listExchangeAccounts();
  }

  if (msg.type === "ADD_EXCHANGE_ACCOUNT" && "payload" in msg) {
    return addExchangeAccount(msg.payload);
  }

  if (msg.type === "DELETE_EXCHANGE_ACCOUNT" && "accountId" in msg) {
    // EXT-16 FR-3: Clear activeExchangeId if deleted account was active
    return deleteExchangeAccount(msg.accountId).then(async (result) => {
      if (result.success) {
        const activeId = await getActiveExchangeId();
        if (activeId === msg.accountId) {
          await setActiveExchangeId(null);
        }
      }
      return result;
    });
  }

  if (msg.type === "TEST_EXCHANGE_CONNECTION" && "accountId" in msg) {
    return testExchangeConnection(msg.accountId);
  }

  // EXT-16 FR-3: Active exchange selection
  if (msg.type === "GET_ACTIVE_EXCHANGE") {
    return getActiveExchangeId().then((id) => ({ exchangeId: id }));
  }

  if (msg.type === "SET_ACTIVE_EXCHANGE" && "exchangeId" in msg) {
    return setActiveExchangeId(msg.exchangeId).then(() => ({ success: true }));
  }

  // EXT-16 FR-2: Sidecar health status
  if (msg.type === "SIDECAR_STATUS") {
    return Promise.resolve({ status: sidecarStatus });
  }
});

// On startup, schedule token refresh if tokens exist, then connect WebSocket
getTokens().then((tokens) => {
  if (tokens && tokens.expires_in > 0) {
    scheduleTokenRefresh(tokens.expires_in);
  }
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
