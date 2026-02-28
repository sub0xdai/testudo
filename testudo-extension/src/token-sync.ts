// Token sync content script — runs on web app pages to sync JWT tokens to extension storage.
// Bridges localStorage (web app) → chrome.storage.local (extension).

import browser from "webextension-polyfill";

const TOKEN_KEYS = ["access_token", "refresh_token"] as const;

function decodeTokenExpiry(accessToken: string): number {
  try {
    const payload = JSON.parse(atob(accessToken.split(".")[1])) as { exp?: number };
    return payload.exp || 0;
  } catch {
    return 0;
  }
}

async function syncTokensToExtension(): Promise<void> {
  const accessToken = localStorage.getItem("access_token");
  const refreshToken = localStorage.getItem("refresh_token");

  if (!accessToken || !refreshToken) return;

  const tokenExpiry = decodeTokenExpiry(accessToken);
  if (tokenExpiry <= Math.floor(Date.now() / 1000)) return;

  await browser.storage.local.set({
    accessToken,
    refreshToken,
    tokenExpiry,
  });

  browser.runtime.sendMessage({ type: "TOKEN_SYNCED_FROM_WEB" }).catch(() => {});
}

async function clearExtensionTokens(): Promise<void> {
  await browser.storage.local.remove(["accessToken", "refreshToken", "tokenExpiry"]);
  browser.runtime.sendMessage({ type: "LOGOUT" }).catch(() => {});
}

// Sync on page load
syncTokensToExtension();

// Listen for cross-tab storage changes
window.addEventListener("storage", (e: StorageEvent) => {
  if (e.key && TOKEN_KEYS.includes(e.key as typeof TOKEN_KEYS[number])) {
    if (e.newValue) {
      syncTokensToExtension();
    } else {
      clearExtensionTokens();
    }
  }
});

// Monkey-patch localStorage to catch same-tab changes (login/logout)
const originalSetItem = localStorage.setItem.bind(localStorage);
const originalRemoveItem = localStorage.removeItem.bind(localStorage);

localStorage.setItem = function (key: string, value: string): void {
  originalSetItem(key, value);
  if (TOKEN_KEYS.includes(key as typeof TOKEN_KEYS[number])) {
    syncTokensToExtension();
  }
};

localStorage.removeItem = function (key: string): void {
  originalRemoveItem(key);
  if (TOKEN_KEYS.includes(key as typeof TOKEN_KEYS[number])) {
    const accessToken = localStorage.getItem("access_token");
    const refreshToken = localStorage.getItem("refresh_token");
    if (!accessToken || !refreshToken) {
      clearExtensionTokens();
    }
  }
};
