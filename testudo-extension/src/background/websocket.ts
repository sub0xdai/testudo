/** @anchor api:ext-bg:websocket
 * @tags api */

import browser from "webextension-polyfill";
import type { WsState } from "../types";
import {
  WS_BASE_RECONNECT_DELAY,
  nextReconnectDelay,
} from "../utils";
import {
  JwtSubPayloadSchema,
  SidecarStreamDataSchema,
  WebSocketMessageSchema,
} from "../schemas";
import { getSettings } from "./storage";
import { getTokens } from "./auth";

// --- Sidecar Health Callback ---
// Injected by bootstrap to avoid circular dep with sidecar module.

type SidecarHealthHandler = (status: "healthy" | "unreachable") => void;
let sidecarHealthHandler: SidecarHealthHandler | null = null;

export function onSidecarHealth(handler: SidecarHealthHandler): void {
  sidecarHealthHandler = handler;
}

// --- WebSocket State ---

let ws: WebSocket | null = null;
let wsState: WsState = "disconnected";
let wsReconnectDelay = 1000;
let wsReconnectTimer: ReturnType<typeof setTimeout> | null = null;
let wsSubscriptionId = 1;

export function getWsState(): WsState {
  return wsState;
}

export function getWsReconnectTimer(): ReturnType<typeof setTimeout> | null {
  return wsReconnectTimer;
}

export function resetReconnectDelay(): void {
  wsReconnectDelay = WS_BASE_RECONNECT_DELAY;
}

function setWsState(state: WsState): void {
  wsState = state;
  browser.runtime.sendMessage({ type: "WS_STATE_CHANGED", state }).catch(() => {});
}

// --- User ID from JWT ---

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

// --- WebSocket Connection (EXT-06) ---

export async function connectWebSocket(): Promise<void> {
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
        sidecarHealthHandler?.(data.success && data.data.status === "healthy" ? "healthy" : "unreachable");
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

export function disconnectWebSocket(): void {
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

// --- Content Tab Cache ---

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

// --- Debounced Reconnect ---

let wsDebounceTimer: ReturnType<typeof setTimeout> | null = null;

export function debouncedConnectWebSocket(): void {
  if (wsDebounceTimer) clearTimeout(wsDebounceTimer);
  wsDebounceTimer = setTimeout(() => {
    wsDebounceTimer = null;
    connectWebSocket();
  }, 300);
}
