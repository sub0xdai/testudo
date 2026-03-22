import browser from "webextension-polyfill";
import type { Settings } from "../types";
import { DEFAULT_SETTINGS } from "../utils";
import type { ExchangeMode } from "../utils";
import { StoredSettingsSchema, SettingsSchema } from "../schemas";

// --- Settings & Storage Helpers ---

export async function getSettings(): Promise<Settings> {
  const stored = await browser.storage.local.get(["backendUrl", "wsUrl"]);
  const parsed = StoredSettingsSchema.safeParse(stored);

  if (!parsed.success) {
    return { ...DEFAULT_SETTINGS };
  }

  const candidate = {
    backendUrl: parsed.data.backendUrl || DEFAULT_SETTINGS.backendUrl,
    wsUrl: parsed.data.wsUrl || DEFAULT_SETTINGS.wsUrl,
  };

  const validated = SettingsSchema.safeParse(candidate);
  if (!validated.success) {
    return { ...DEFAULT_SETTINGS };
  }

  return validated.data;
}

// --- Active Exchange Selection (EXT-32: per-mode active exchange) ---

export async function getExchangeMode(): Promise<ExchangeMode> {
  const stored = await browser.storage.local.get("exchangeMode");
  const mode = stored.exchangeMode;
  return mode === "dex" ? "dex" : "cex";
}

export async function getActiveExchangeId(): Promise<string | null> {
  const mode = await getExchangeMode();
  const key = mode === "dex" ? "activeDexAccountId" : "activeCexAccountId";
  const stored = await browser.storage.local.get(key);
  return (stored[key] as string) || null;
}

export async function setActiveExchangeId(id: string | null): Promise<void> {
  const mode = await getExchangeMode();
  const key = mode === "dex" ? "activeDexAccountId" : "activeCexAccountId";
  if (id) {
    await browser.storage.local.set({ [key]: id });
  } else {
    await browser.storage.local.remove([key]);
  }
}
