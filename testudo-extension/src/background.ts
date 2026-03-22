import browser from "webextension-polyfill";
import type { WsState } from "./types";
import {
  WS_BASE_RECONNECT_DELAY,
  nextReconnectDelay,
} from "./utils";
import {
  JwtSubPayloadSchema,
  RuntimeMessageSchema,
  SidecarHealthResponseSchema,
  SidecarStreamDataSchema,
  WebSocketMessageSchema,
} from "./schemas";
import { getSettings, getExchangeMode, getActiveExchangeId, setActiveExchangeId } from "./background/storage";
import { getTokens, clearTokens, refreshAccessToken, scheduleTokenRefresh, clearRefreshTimer, getAuthStatus } from "./background/auth";
import {
  apiRequest,
  login,
  register,
  forgotPassword,
  ensureActiveExchange,
  migrateActiveExchangeId,
  executeTrade,
  listTrades,
  cancelTrade,
  cleanupTrades,
  listExchanges,
  listExchangeAccounts,
  addExchangeAccount,
  deleteExchangeAccount,
  testExchangeConnection,
  getLiveBalance,
  fetchExchangePositions,
  closeExchangePosition,
} from "./background/api";

// Background service worker — manages settings, auth, REST dispatch, and WebSocket connection.

// AUD-07 FR-8: Global error logging for unhandled promise rejections
self.addEventListener("unhandledrejection", (event: PromiseRejectionEvent) => {
  console.error("Unhandled promise rejection:", event.reason);
});

browser.runtime.onInstalled.addListener(async () => {
  const settings = await getSettings();
  // EXT-19: Clean up legacy paper trading storage keys
  await browser.storage.local.remove(["executionMode", "paperOnly"]);
  await browser.storage.local.set({ ...settings });
  console.log("Testudo Sniper installed", settings);
});

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
  clearRefreshTimer();
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
