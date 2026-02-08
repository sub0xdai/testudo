import browser from "webextension-polyfill";
import { scrapeTradeSetup } from "./scraper";
import type { TradeSetup } from "./scraper";

console.log("Testudo Sniper loaded");

// Content script entry point — injected on TradingView chart pages.
// EXT-03 will add the hotkey listener and confirmation modal here.

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

// Re-export for use by other modules in the content script context
export type { TradeSetup };
export { scrapeTradeSetup };
