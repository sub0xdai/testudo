import browser from "webextension-polyfill";
import { RuntimeMessageSchema } from "./schemas";
import { getSettings } from "./background/storage";
import { getTokens, scheduleTokenRefresh } from "./background/auth";
import { ensureActiveExchange, migrateActiveExchangeId } from "./background/api";
import { connectWebSocket, disconnectWebSocket, debouncedConnectWebSocket, onSidecarHealth } from "./background/websocket";
import { setSidecarStatus, startSidecarHealthPolling } from "./background/sidecar";
import { messageHandlers } from "./background/handlers";

// Background service worker — thin bootstrap that wires modules and starts services.

// AUD-07 FR-8: Global error logging for unhandled promise rejections
self.addEventListener("unhandledrejection", (event: PromiseRejectionEvent) => {
  console.error("Unhandled promise rejection:", event.reason);
});

browser.runtime.onInstalled.addListener(async () => {
  const settings = await getSettings();
  // EXT-19: Clean up legacy paper trading storage keys
  await browser.storage.local.remove(["executionMode", "paperOnly"]);
  await browser.storage.local.set({ ...settings });
  console.log("Testudo installed", settings);
});

// --- EXT-46: Browser-level Alt+X shortcut (bypasses all page event interception) ---

browser.commands.onCommand.addListener(async (command: string) => {
  if (command === "trigger-trade") {
    const tabs = await browser.tabs.query({ active: true, currentWindow: true });
    if (tabs[0]?.id) {
      browser.tabs.sendMessage(tabs[0].id, { type: "TRIGGER_ALT_X" }).catch(() => {});
    }
  }
});

// --- Message Dispatch ---

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

// Wire sidecar health updates from WebSocket to sidecar status
onSidecarHealth(setSidecarStatus);

// EXT-06: Connect WebSocket on startup
connectWebSocket();

// EXT-16: Start sidecar health polling
startSidecarHealthPolling();

// Reconnect WebSocket when settings change (debounced to collapse rapid changes)
browser.storage.onChanged.addListener((changes) => {
  if (changes.wsUrl) {
    debouncedConnectWebSocket();
  }
});

// Export for testing — unused at runtime, tree-shaken by esbuild
export { disconnectWebSocket as _disconnectWebSocket };
