import browser from "webextension-polyfill";
import type { RuntimeMessageSchema } from "../schemas";
import { getSettings, getExchangeMode, getActiveExchangeId, setActiveExchangeId } from "./storage";
import { getTokens, clearTokens, refreshAccessToken, clearRefreshTimer, getAuthStatus } from "./auth";
import {
  pair,
  ensureActiveExchange,
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
  listSetupTags,
} from "./api";
import {
  connectWebSocket,
  disconnectWebSocket,
  getWsState,
  getWsReconnectTimer,
  resetReconnectDelay,
} from "./websocket";
import { getSidecarStatus, stopSidecarHealthPolling } from "./sidecar";

// --- Types ---

type ParsedMessage = ReturnType<typeof RuntimeMessageSchema.parse>;
type MessageHandler = (msg: ParsedMessage) => Promise<unknown> | unknown;
type MsgOf<T extends ParsedMessage["type"]> = Extract<ParsedMessage, { type: T }>;

// --- Handler Functions ---

function handleGetSettings(): Promise<unknown> {
  return getSettings();
}

function handleExecuteTrade(msg: ParsedMessage): Promise<unknown> {
  return executeTrade((msg as MsgOf<"EXECUTE_TRADE">).payload);
}

function handlePair(msg: ParsedMessage): Promise<unknown> {
  const { code } = msg as MsgOf<"PAIR">;
  return pair(code).then(async (result) => {
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
  const state = getWsState();
  // Auto-reconnect if disconnected when popup queries status
  if (state === "disconnected" && !getWsReconnectTimer()) {
    resetReconnectDelay();
    connectWebSocket();
  }
  return Promise.resolve({ state });
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

function handleSidecarStatus(): Promise<unknown> {
  return Promise.resolve({ status: getSidecarStatus() });
}

function handleExchangePositions(): Promise<unknown> {
  return fetchExchangePositions();
}

function handleCloseExchangePosition(msg: ParsedMessage): Promise<unknown> {
  const { symbol, side, contracts } = msg as MsgOf<"CLOSE_EXCHANGE_POSITION">;
  return closeExchangePosition(symbol, side, contracts);
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

function handleGetSetupTags(msg: ParsedMessage): Promise<unknown> {
  return listSetupTags((msg as MsgOf<"GET_SETUP_TAGS">).limit);
}

// --- Dispatch Map ---

export const messageHandlers: Record<string, MessageHandler> = {
  GET_SETTINGS: handleGetSettings,
  EXECUTE_TRADE: handleExecuteTrade,
  PAIR: handlePair,
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
  SIDECAR_STATUS: handleSidecarStatus,
  EXCHANGE_POSITIONS: handleExchangePositions,
  CLOSE_EXCHANGE_POSITION: handleCloseExchangePosition,
  GET_EXCHANGE_MODE: handleGetExchangeMode,
  SET_EXCHANGE_MODE: handleSetExchangeMode,
  ACCOUNT_LINKED: handleAccountLinked,
  GET_SETUP_TAGS: handleGetSetupTags,
};
