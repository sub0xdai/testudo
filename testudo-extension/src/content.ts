import { scrapeTradeSetup, scrapeSymbol } from "./scraper";

// MV3 provides Promise-based APIs natively — no polyfill needed in content scripts
const browser = (globalThis as any).browser ?? (globalThis as any).chrome;
import type { TradeSetup } from "./scraper";
import { showModal, showOrderToast, showToast, isVisible } from "./modal";
import type { ModalResult } from "./modal";
import type { ManagementPreset, BalanceResponse, LiveBalanceResponse } from "./types";
import { DEFAULT_MANAGEMENT_PRESET } from "./types";

console.log("Testudo Sniper loaded");

// --- Platform Detection ---

function isTradingView(): boolean {
  return location.hostname.includes("tradingview.com");
}

function isDexScreener(): boolean {
  return location.hostname.includes("dexscreener.com");
}

function isHyperliquid(): boolean {
  return location.hostname.includes("hyperliquid");
}

function isChartPlatform(): boolean {
  const host = location.hostname;
  return host.includes("tradingview.com")
    || host.includes("dexscreener.com")
    || isHyperliquid()
    || host.includes("gmx.io")
    || host.includes("bybit.com");
}

// --- Main-World Bridge (EXT-43) ---

let bridgeReady = false;
let bridgeRequestId = 0;

function injectBridge(): void {
  if (document.getElementById("testudo-bridge")) return;
  const script = document.createElement("script");
  script.id = "testudo-bridge";
  script.src = browser.runtime.getURL("page-bridge.js");
  (document.head || document.documentElement).appendChild(script);
}

// Listen for bridge ready signal
window.addEventListener("message", (event: MessageEvent) => {
  if (event.source !== window) return;
  if (event.data?.type === "TESTUDO_BRIDGE_READY") {
    bridgeReady = true;
  }
});

function bridgeRequest(action: "probe" | "getPositionTool" | "getSymbol"): Promise<any> {
  return new Promise((resolve) => {
    if (!bridgeReady) {
      resolve(null);
      return;
    }

    const id = ++bridgeRequestId;
    const timeout = setTimeout(() => {
      window.removeEventListener("message", handler);
      resolve(null);
    }, 500);

    function handler(event: MessageEvent) {
      if (event.source !== window) return;
      if (event.data?.type !== "TESTUDO_BRIDGE_RESPONSE") return;
      if (event.data.id !== id) return;

      clearTimeout(timeout);
      window.removeEventListener("message", handler);
      resolve(event.data.data);
    }

    window.addEventListener("message", handler);
    window.postMessage({ type: "TESTUDO_BRIDGE_REQUEST", action, id }, "*");
  });
}

// Inject bridge on chart platforms
if (isChartPlatform()) {
  injectBridge();
}

// --- Management Preset Loader ---

async function getManagementPreset(): Promise<ManagementPreset> {
  try {
    const storage = browser?.storage?.local;
    if (!storage) {
      return { ...DEFAULT_MANAGEMENT_PRESET };
    }
    const stored = await storage.get(["managementPreset"]);
    return (stored.managementPreset as ManagementPreset) || { ...DEFAULT_MANAGEMENT_PRESET };
  } catch {
    return { ...DEFAULT_MANAGEMENT_PRESET };
  }
}

// --- Hotkey Listener ---

let altXPending = false;

document.addEventListener("keydown", async (e: KeyboardEvent) => {
  if (e.altKey && e.key.toLowerCase() === "x" && !isVisible() && !altXPending) {
    altXPending = true;
    e.preventDefault();
    e.stopPropagation();

    try {
      let setup: TradeSetup | null = null;

      // Try main-world bridge first (works on all chart platforms)
      if (bridgeReady) {
        const bridgeData = await bridgeRequest("getPositionTool");
        if (bridgeData && bridgeData.entry > 0) {
          // Get symbol via bridge, fall back to DOM scrape
          let symbol = await bridgeRequest("getSymbol");
          if (!symbol) symbol = scrapeSymbol();
          if (symbol) {
            const timeframe = "chart"; // bridge doesn't provide timeframe, label as chart-sourced
            setup = { symbol, side: bridgeData.side, entry: bridgeData.entry, stop: bridgeData.stop, target: bridgeData.target, timeframe };
          }
        }
      }

      // Fall back to DOM-based scraper strategies
      if (!setup) {
        const strategiesToTry = isTradingView() ? undefined : [2];
        setup = scrapeTradeSetup(strategiesToTry);
      }

      // Fallback: try symbol-only detection on chart platforms
      if (!setup && isChartPlatform()) {
        // Try bridge symbol first, then DOM
        let symbol = bridgeReady ? await bridgeRequest("getSymbol") : null;
        if (!symbol) symbol = scrapeSymbol();
        if (symbol) {
          setup = { symbol, side: "LONG", entry: 0, stop: 0, target: 0, timeframe: "manual" } as TradeSetup;
        }
      }

      const management = await getManagementPreset();

      // Fetch live balance from active exchange
      let balance: BalanceResponse[] | null = null;
      try {
        const resp = await browser.runtime.sendMessage({ type: "GET_BALANCE" }) as {
          success: boolean; data?: LiveBalanceResponse;
        };
        balance = resp?.success && resp.data ? resp.data.balances : null;
      } catch { /* non-blocking */ }

      // Fetch active exchange name for modal badge
      let activeExchangeName: string | null = null;
      try {
        const activeRes = await browser.runtime.sendMessage({ type: "GET_ACTIVE_EXCHANGE" }) as { exchangeId: string | null };
        if (activeRes?.exchangeId) {
          const accountsRes = await browser.runtime.sendMessage({ type: "LIST_EXCHANGE_ACCOUNTS" }) as { success: boolean; data?: Array<{ id: string; exchange_name: string; account_name: string }> };
          if (accountsRes?.success && accountsRes.data) {
            const active = accountsRes.data.find(a => a.id === activeRes.exchangeId);
            activeExchangeName = active?.account_name || active?.exchange_name || null;
          }
        }
      } catch { /* non-blocking */ }

      // Convert symbol-only setup to proper initialSetup for the form
      const modalSetup = setup && setup.entry > 0 ? setup : null;
      const symbolHint = setup?.symbol || null;

      // If we only have a symbol, create a partial setup for the modal
      const initialSetup = modalSetup ?? (symbolHint ? {
        symbol: symbolHint,
        side: "LONG" as const,
        entry: 0,
        stop: 0,
        target: 0,
        timeframe: "manual",
      } : null);

      // Read theme before opening modal (sync with extension popup)
      let theme: string | undefined;
      try {
        const stored = await browser.storage.local.get("testudo-theme");
        theme = stored["testudo-theme"] as string | undefined;
      } catch { /* default dark */ }

      showModal(initialSetup, management, handleModalResult, balance, activeExchangeName, theme);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes("Extension context invalidated")) {
        showToast("Extension updated — refresh this page", "error");
      } else {
        showToast(`Error: ${msg}`, "error");
      }
    } finally {
      altXPending = false;
    }
  }
}, true);

function handleModalResult(result: ModalResult, setup: TradeSetup | null): void {
  if (result === "confirm" && setup) {
    executeTrade(setup);
  }
}

// --- Trade Execution ---

async function executeTrade(setup: TradeSetup): Promise<void> {
  const management = await getManagementPreset();

  try {
    const response = await browser.runtime.sendMessage({
      type: "EXECUTE_TRADE",
      payload: {
        ...setup,
        management: {
          risk_percent: management.risk_percent,
          break_even_enabled: management.break_even_enabled,
          break_even_at: management.break_even_at,
          leverage: management.leverage,
          trailing_stop: management.trailing_stop,
          partial_tp: management.partial_tp,
        },
      },
    }) as { success: boolean; data?: unknown; error?: string; warnings?: string[] };

    if (response.success) {
      if (response.warnings && response.warnings.length > 0) {
        showToast(`WARNING: ${response.warnings.join("; ")}`, "error");
      } else {
        showToast("Order Sent", "success");
      }
    } else {
      showToast(`Error: ${response.error || "Unknown error"}`, "error");
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : "Failed to send trade";
    showToast(`Error: ${msg}`, "error");
  }
}

// --- Message Listener ---

browser.runtime.onMessage.addListener((message: unknown) => {
  const msg = message as { type: string; data?: Record<string, unknown> };
  if (msg.type === "PING") {
    return Promise.resolve({ status: "alive" });
  }
  if (msg.type === "SCRAPE") {
    const setup = scrapeTradeSetup();
    return Promise.resolve(setup);
  }
  if (msg.type === "WS_ORDER_UPDATE" && msg.data) {
    const event = (msg.data.e as string) || "order";
    const symbol = (msg.data.s as string) || "";
    const status = (msg.data.status as string) || "";
    const eventType = `order.${status || event}`;
    const label = status ? `${event}: ${symbol} ${status}` : `${event}: ${symbol}`;
    showOrderToast(eventType, label);
  }
});

export type { TradeSetup };
export { scrapeTradeSetup, executeTrade };
