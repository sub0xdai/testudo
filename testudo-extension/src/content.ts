import browser from "webextension-polyfill";
import { scrapeTradeSetup } from "./scraper";
import type { TradeSetup } from "./scraper";
import { showModal, showToast, isVisible, dismiss } from "./modal";
import type { ModalResult } from "./modal";

console.log("Testudo Sniper loaded");

// --- Hotkey Listener ---
// Default: Alt+X. EXT-03 FR-7 (configurable hotkey) is low priority, deferred.

document.addEventListener("keydown", (e: KeyboardEvent) => {
  if (e.altKey && e.key.toLowerCase() === "x" && !isVisible()) {
    e.preventDefault();
    e.stopPropagation();

    const setup = scrapeTradeSetup();
    showModal(setup, handleModalResult);
  }
}, true);

function handleModalResult(result: ModalResult, setup: TradeSetup | null): void {
  if (result === "confirm" && setup) {
    executeTrade(setup);
  }
}

// --- Trade Execution (wired in EXT-04) ---

async function executeTrade(setup: TradeSetup): Promise<void> {
  // EXT-04 will implement REST dispatch here.
  // For now, log the trade setup.
  console.log("[Testudo] Trade confirmed:", setup);
  showToast("Trade execution not yet connected", "error");
}

// --- Message Listener ---

browser.runtime.onMessage.addListener((message: unknown) => {
  const msg = message as { type: string };
  if (msg.type === "PING") {
    return Promise.resolve({ status: "alive" });
  }
  if (msg.type === "SCRAPE") {
    const setup = scrapeTradeSetup();
    return Promise.resolve(setup);
  }
});

export type { TradeSetup };
export { scrapeTradeSetup, executeTrade };
