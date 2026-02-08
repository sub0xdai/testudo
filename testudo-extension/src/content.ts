import browser from "webextension-polyfill";
import { scrapeTradeSetup } from "./scraper";
import type { TradeSetup } from "./scraper";
import { showModal, showToast, isVisible } from "./modal";
import type { ModalResult } from "./modal";

console.log("Testudo Sniper loaded");

// --- Hotkey Listener ---
// Default: Alt+X. EXT-03 FR-7 (configurable hotkey) is low priority, deferred.

document.addEventListener("keydown", async (e: KeyboardEvent) => {
  if (e.altKey && e.key.toLowerCase() === "x" && !isVisible()) {
    e.preventDefault();
    e.stopPropagation();

    const setup = scrapeTradeSetup();

    // EXT-05: Check execution mode for LIVE warning
    const settings = await browser.runtime.sendMessage({ type: "GET_SETTINGS" }) as {
      executionMode: "paper" | "live";
    };

    showModal(setup, settings.executionMode === "live", handleModalResult);
  }
}, true);

function handleModalResult(result: ModalResult, setup: TradeSetup | null): void {
  if (result === "confirm" && setup) {
    executeTrade(setup);
  }
}

// --- Trade Execution (EXT-04 + EXT-05: REST dispatch via background worker) ---

async function executeTrade(setup: TradeSetup): Promise<void> {
  try {
    const response = await browser.runtime.sendMessage({
      type: "EXECUTE_TRADE",
      payload: setup,
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
  // EXT-06 FR-6: Real-time order updates via WebSocket
  if (msg.type === "WS_ORDER_UPDATE" && msg.data) {
    const event = (msg.data.e as string) || "order";
    const symbol = (msg.data.s as string) || "";
    const status = (msg.data.status as string) || "";
    const label = status ? `${event}: ${symbol} ${status}` : `${event}: ${symbol}`;
    showToast(label, "success");
  }
});

export type { TradeSetup };
export { scrapeTradeSetup, executeTrade };
