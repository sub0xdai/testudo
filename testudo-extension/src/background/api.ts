import browser from "webextension-polyfill";
import type {
  TradePayload,
  BackendResponse,
  TradeGroupResponse,
  BalanceResponse,
  LiveBalanceResponse,
  ExchangeInfo,
  ExchangeAccount,
  AddExchangeAccountPayload,
  TestConnectionResult,
  ExchangePositionsResponse,
} from "../types";
import { normalizeSymbol, mapSide, getExchangeType } from "../utils";
import {
  AddExchangeAccountResponseSchema,
  BackendResponseSchema,
  ErrorResponseSchema,
  ExchangeAccountsResponseSchema,
  ExchangeBalanceApiResponseSchema,
  ExchangePositionsApiResponseSchema,
  ListExchangesResponseSchema,
  PairResponseSchema,
  TestConnectionResultSchema,
  TradeGroupResponseSchema,
  TradeListResponseSchema,
} from "../schemas";
import { getSettings, getExchangeMode, getActiveExchangeId, setActiveExchangeId } from "./storage";
import { getTokens, storeTokens, refreshAccessToken, scheduleTokenRefresh } from "./auth";

// --- Types ---

type RuntimeTradePayload = Omit<TradePayload, "management"> & {
  management: Omit<TradePayload["management"], "leverage"> & { leverage?: number };
};

type AuthMode = "hard" | "soft" | "none";

interface ApiOpts {
  method?: string;
  body?: unknown;
  auth?: AuthMode;
  authError?: string;
  timeout?: number;
}

type ApiResult = { ok: true; raw: unknown } | { ok: false; error: string; error_code?: string; httpError?: boolean };

// --- Normalizers ---

export function normalizeBackendAck(raw: unknown): BackendResponse {
  if (raw && typeof raw === "object") {
    const obj = raw as Record<string, unknown>;
    const dataObj = obj.data && typeof obj.data === "object" ? obj.data as Record<string, unknown> : null;
    const warnings = Array.isArray(dataObj?.warnings) ? dataObj.warnings as string[] : undefined;
    const error_code = typeof obj.error_code === "string" ? obj.error_code : undefined;

    if (typeof obj.success === "boolean") {
      return {
        success: obj.success,
        data: obj.data,
        error: typeof obj.error === "string" || obj.error === null ? obj.error : null,
        error_code,
        warnings,
      };
    }

    if (typeof obj.error === "string") {
      return { success: false, data: null, error: obj.error, error_code };
    }

    if (typeof obj.message === "string") {
      return { success: false, data: null, error: obj.message, error_code };
    }

    return { success: true, data: raw, error: null, warnings };
  }

  return { success: true, data: raw, error: null };
}

export function normalizeTradeListResponse(raw: unknown): BackendResponse {
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

// --- Shared API Request Helper ---

export async function apiRequest(
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
      const errorCode = json.success ? json.data.error_code : undefined;
      return { ok: false, error: errorMsg || `HTTP ${response.status}`, error_code: errorCode, httpError: true };
    }

    const raw = await response.json().catch(() => ({}));
    return { ok: true, raw };
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Network error";
    return { ok: false, error: msg };
  }
}

// --- Auth: Device Pairing ---

export async function pair(code: string): Promise<{ success: boolean; error?: string }> {
  const result = await apiRequest("/api/v1/auth/extension-pair", {
    method: "POST", body: { code },
  });
  if (!result.ok) return { success: false, error: result.error };
  const parsed = PairResponseSchema.safeParse(result.raw);
  if (!parsed.success) return { success: false, error: "Unexpected server response" };
  await storeTokens(parsed.data.tokens);
  scheduleTokenRefresh(parsed.data.tokens.expires_in);
  return { success: true };
}

// --- Exchange Account Management ---

export async function ensureActiveExchange(): Promise<string | null> {
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
export async function migrateActiveExchangeId(): Promise<void> {
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

export async function executeTrade(payload: RuntimeTradePayload): Promise<BackendResponse> {
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
    if (!result.ok) return { success: false, error: result.error, error_code: result.error_code };

    const normalized = normalizeBackendAck(result.raw);
    const validated = BackendResponseSchema.safeParse(normalized);
    if (!validated.success) return { success: false, error: "Malformed trade response" };
    return validated.data;
  } finally {
    tradeInFlight = false;
  }
}

// --- Trade Listing (Active Orders) ---

export async function listTrades(): Promise<{ success: boolean; data?: TradeGroupResponse[]; error?: string }> {
  const result = await apiRequest("/api/v1/trades", { auth: "hard" });
  if (!result.ok) return { success: false, error: result.error };

  const normalized = normalizeTradeListResponse(result.raw);
  const validated = TradeListResponseSchema.safeParse(normalized);
  if (!validated.success) return { success: false, error: "Malformed trade list response" };
  if (!validated.data.success) return { success: false, error: validated.data.error || "Trade list request failed" };
  return { success: true, data: validated.data.data || [] };
}

export async function cancelTrade(tradeId: string): Promise<BackendResponse> {
  const result = await apiRequest(`/api/v1/trades/${tradeId}`, {
    method: "DELETE", auth: "hard",
  });
  if (!result.ok) return { success: false, error: result.error };

  const normalized = normalizeBackendAck(result.raw);
  const validated = BackendResponseSchema.safeParse(normalized);
  if (!validated.success) return { success: false, error: "Malformed cancel response" };
  return validated.data;
}

export async function cleanupTrades(): Promise<BackendResponse> {
  const result = await apiRequest("/api/v1/trades/cleanup", {
    method: "POST", auth: "hard",
  });
  if (!result.ok) return { success: false, error: result.error };
  return { success: true };
}

// --- Exchange Listing ---

export async function listExchanges(): Promise<{ success: boolean; data?: ExchangeInfo[]; error?: string }> {
  const result = await apiRequest("/api/v1/exchanges", { auth: "soft" });
  if (!result.ok) return { success: false, error: result.error };

  const json = ListExchangesResponseSchema.safeParse(result.raw);
  if (!json.success) return { success: false, error: "Malformed exchanges response" };
  return { success: true, data: json.data.exchanges || [] };
}

export async function listExchangeAccounts(): Promise<{ success: boolean; data?: ExchangeAccount[]; error?: string }> {
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

export async function addExchangeAccount(payload: AddExchangeAccountPayload): Promise<{ success: boolean; data?: ExchangeAccount; error?: string }> {
  const result = await apiRequest("/api/v1/exchanges/accounts", {
    method: "POST", body: payload, auth: "soft",
  });
  if (!result.ok) return { success: false, error: result.error };

  const json = AddExchangeAccountResponseSchema.safeParse(result.raw);
  if (!json.success) return { success: false, error: "Malformed add account response" };
  return { success: true, data: json.data.data };
}

export async function deleteExchangeAccount(accountId: string): Promise<{ success: boolean; error?: string }> {
  const result = await apiRequest(`/api/v1/exchanges/accounts/${accountId}`, {
    method: "DELETE", auth: "soft",
  });
  if (!result.ok) return { success: false, error: result.error };
  return { success: true };
}

export async function testExchangeConnection(accountId: string): Promise<{ success: boolean; data?: TestConnectionResult; error?: string }> {
  const result = await apiRequest(`/api/v1/exchanges/accounts/${accountId}/test`, {
    method: "POST", auth: "soft",
  });
  if (!result.ok) return { success: false, error: result.error };

  const json = TestConnectionResultSchema.safeParse(result.raw);
  if (!json.success) return { success: false, error: "Malformed connection test response" };
  return { success: true, data: json.data };
}

// --- EXT-19: Live Exchange Balance ---

export async function getLiveBalance(): Promise<{ success: boolean; data?: LiveBalanceResponse; error?: string }> {
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
    asset: b.asset, total: b.total, available: b.free, locked: b.used,
  }));
  return { success: true, data: { exchange_name: json.data.exchange_name, balances } };
}

// --- Exchange Positions ---

export async function fetchExchangePositions(): Promise<{ success: boolean; data?: ExchangePositionsResponse; error?: string }> {
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

export async function closeExchangePosition(
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
