import browser from "webextension-polyfill";
import type { AuthTokens } from "../types";
import { calculateRefreshDelay } from "../utils";
import {
  AuthTokensSchema,
  JwtWalletPayloadSchema,
  RefreshResponseSchema,
  StoredTokensSchema,
} from "../schemas";
import { getSettings } from "./storage";

// --- Auth Token Management (EXT-05 FR-2, FR-3, FR-7) ---

export async function getTokens(): Promise<AuthTokens | null> {
  const stored = await browser.storage.session.get(["accessToken", "refreshToken", "tokenExpiry"]);
  const parsed = StoredTokensSchema.safeParse(stored);
  if (!parsed.success) return null;

  const tokens = {
    access_token: parsed.data.accessToken,
    refresh_token: parsed.data.refreshToken,
    expires_in: (parsed.data.tokenExpiry || 0) - Math.floor(Date.now() / 1000),
  };

  const validated = AuthTokensSchema.safeParse(tokens);
  return validated.success ? validated.data : null;
}

export async function storeTokens(tokens: AuthTokens): Promise<void> {
  await browser.storage.session.set({
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token,
    tokenExpiry: Math.floor(Date.now() / 1000) + tokens.expires_in,
  });
}

export async function clearTokens(): Promise<void> {
  await browser.storage.session.remove(["accessToken", "refreshToken", "tokenExpiry"]);
}

// --- Token Refresh ---

let refreshInFlight: Promise<boolean> | null = null;

export async function refreshAccessToken(): Promise<boolean> {
  if (refreshInFlight) return refreshInFlight;

  refreshInFlight = doRefresh();
  try {
    return await refreshInFlight;
  } finally {
    refreshInFlight = null;
  }
}

// Uses raw fetch (not apiRequest) to break auth ↔ api circular dependency.
// The refresh endpoint only needs the refresh token in the body — no JWT auth header.
async function doRefresh(): Promise<boolean> {
  const tokens = await getTokens();
  if (!tokens) return false;

  const settings = await getSettings();

  try {
    const response = await fetch(`${settings.backendUrl}/api/v1/auth/extension-refresh`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ refresh_token: tokens.refresh_token }),
    });

    if (!response.ok) {
      await clearTokens();
      return false;
    }

    const raw = await response.json().catch(() => ({}));
    const parsed = RefreshResponseSchema.safeParse(raw);
    if (!parsed.success) return false;

    await storeTokens(parsed.data.tokens);
    scheduleTokenRefresh(parsed.data.tokens.expires_in);
    return true;
  } catch {
    return false;
  }
}

// --- Scheduled Refresh ---

let refreshTimer: ReturnType<typeof setTimeout> | null = null;

export function scheduleTokenRefresh(expiresIn: number): void {
  if (refreshTimer) clearTimeout(refreshTimer);
  const refreshDelay = calculateRefreshDelay(expiresIn);
  refreshTimer = setTimeout(() => {
    refreshAccessToken();
  }, refreshDelay);
}

export function clearRefreshTimer(): void {
  if (refreshTimer) {
    clearTimeout(refreshTimer);
    refreshTimer = null;
  }
}

// --- Auth Status ---

export async function getAuthStatus(): Promise<{ authenticated: boolean; walletAddress?: string }> {
  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { authenticated: false };
  }
  try {
    const payloadRaw = JSON.parse(atob(tokens.access_token.split(".")[1] || ""));
    const payload = JwtWalletPayloadSchema.safeParse(payloadRaw);
    if (!payload.success) {
      return { authenticated: true };
    }
    return { authenticated: true, walletAddress: payload.data.wallet_address };
  } catch {
    return { authenticated: true };
  }
}
