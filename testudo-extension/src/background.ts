import browser from "webextension-polyfill";

// Background service worker — manages settings and connection state.
// EXT-05 will add JWT token refresh here.
// EXT-06 will add WebSocket connection management here.

interface Settings {
  backendUrl: string;
  executionMode: "paper" | "live";
}

const DEFAULT_SETTINGS: Settings = {
  backendUrl: "http://localhost:8080",
  executionMode: "paper",
};

async function getSettings(): Promise<Settings> {
  const stored = await browser.storage.local.get(["backendUrl", "executionMode"]);
  return {
    backendUrl: (stored.backendUrl as string) || DEFAULT_SETTINGS.backendUrl,
    executionMode: (stored.executionMode as Settings["executionMode"]) || DEFAULT_SETTINGS.executionMode,
  };
}

browser.runtime.onInstalled.addListener(async () => {
  const settings = await getSettings();
  await browser.storage.local.set({ ...settings });
  console.log("Testudo Sniper installed", settings);
});

browser.runtime.onMessage.addListener((message: unknown) => {
  const msg = message as { type: string };
  if (msg.type === "GET_SETTINGS") {
    return getSettings();
  }
});
