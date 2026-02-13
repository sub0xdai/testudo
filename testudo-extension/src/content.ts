import browser from "webextension-polyfill";
import { scrapeTradeSetup } from "./scraper";
import type { TradeSetup } from "./scraper";
import { showModal, showOrderToast, showToast, isVisible } from "./modal";
import type { ModalResult } from "./modal";
import type { ManagementPreset, BalanceResponse } from "./types";
import { DEFAULT_MANAGEMENT_PRESET } from "./types";

console.log("Testudo Sniper loaded");

// --- Management Preset Loader ---

async function getManagementPreset(): Promise<ManagementPreset> {
  const stored = await browser.storage.local.get(["managementPreset"]);
  return (stored.managementPreset as ManagementPreset) || { ...DEFAULT_MANAGEMENT_PRESET };
}

// --- Hotkey Listener ---

document.addEventListener("keydown", async (e: KeyboardEvent) => {
  if (e.altKey && e.key.toLowerCase() === "x" && !isVisible()) {
    e.preventDefault();
    e.stopPropagation();

    try {
      const setup = scrapeTradeSetup();
      const settings = await browser.runtime.sendMessage({ type: "GET_SETTINGS" }) as {
        executionMode: "paper" | "live";
      };
      const management = await getManagementPreset();

      let balance: BalanceResponse[] | null = null;
      try {
        const resp = await browser.runtime.sendMessage({ type: "GET_BALANCES" }) as {
          success: boolean; data?: BalanceResponse[];
        };
        balance = resp?.success ? (resp.data ?? null) : null;
      } catch { /* non-blocking */ }

      showModal(setup, settings.executionMode === "live", management, handleModalResult, balance);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes("Extension context invalidated")) {
        showToast("Extension updated — refresh this page", "error");
      } else {
        showToast(`Error: ${msg}`, "error");
      }
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
          break_even_at: management.break_even_at,
          leverage: management.leverage,
          trailing_stop: management.trailing_stop,
          partial_tp: management.partial_tp,
        },
      },
    }) as { success: boolean; data?: unknown; error?: string };

    if (response.success) {
      showToast("Order Sent", "success");
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
