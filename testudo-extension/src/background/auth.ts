/** @anchor api:ext-bg:auth
 * @tags api */

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
  const stored = await browser.storage.local.get(["accessToken", "refreshToken", "tokenExpiry"]);
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
  await browser.storage.local.set({
    accessToken: tokens.access_token,
    refreshToken: tokens.refresh_token,
    tokenExpiry: Math.floor(Date.now() / 1000) + tokens.expires_in,
  });
}

export async function clearTokens(): Promise<void> {
  await browser.storage.local.remove(["accessToken", "refreshToken", "tokenExpiry"]);
}

// --- Session State (transient/lost) ---
// Drives the popup's awareness of *why* it's at the pair screen, instead of
// silently dropping back without explanation.
//
//   "ok"               — happy path, refreshes are succeeding
//   "refresh_retrying" — backend transiently unreachable; tokens kept, retry scheduled
//   "session_lost"     — retries exhausted or revoked; tokens cleared, user must re-pair
//   "wallet_changed"   — web app reported a wallet change/logout; tokens cleared
export type SessionState = "ok" | "refresh_retrying" | "session_lost" | "wallet_changed";

export async function getSessionState(): Promise<SessionState> {
  const stored = await browser.storage.local.get("sessionState");
  const v = stored.sessionState as string | undefined;
  if (v === "refresh_retrying" || v === "session_lost" || v === "wallet_changed") return v;
  return "ok";
}

export async function setSessionState(state: SessionState): Promise<void> {
  if (state === "ok") {
    await browser.storage.local.remove("sessionState");
  } else {
    await browser.storage.local.set({ sessionState: state });
  }
}

// --- Token Refresh ---

// Backoff schedule for transient refresh failures (5xx / 408 / 429 / network).
// After REFRESH_BACKOFFS_MS.length attempts all fail, we give up and clear tokens.
// Resets on every successful refresh and on every fresh pair.
const REFRESH_BACKOFFS_MS = [30_000, 120_000, 480_000]; // 30s, 2min, 8min
let refreshInFlight: Promise<boolean> | null = null;
let consecutiveTransientFailures = 0;

export async function refreshAccessToken(): Promise<boolean> {
  if (refreshInFlight) return refreshInFlight;

  refreshInFlight = doRefresh();
  try {
    return await refreshInFlight;
  } finally {
    refreshInFlight = null;
  }
}

/**
 * Classify a refresh-endpoint response.
 *  - "definitive": refresh token is bad (401/403) or request was malformed (other 4xx).
 *    Clear tokens; user must re-pair. No retry would help.
 *  - "transient":  backend is unreachable (5xx, 408, 429) or fetch threw (offline,
 *    DNS, TLS). Keep tokens; reschedule with backoff. Common during deploys.
 */
type RefreshOutcome = "ok" | "definitive" | "transient";

function classifyRefreshStatus(status: number): RefreshOutcome {
  if (status >= 200 && status < 300) return "ok";
  if (status === 401 || status === 403) return "definitive";
  if (status >= 500 && status < 600) return "transient";
  if (status === 408 || status === 429) return "transient";
  // Other 4xx (400, 404, etc.) → server actively rejected the request shape.
  // Retrying won't fix it. Treat as definitive.
  return "definitive";
}

// Uses raw fetch (not apiRequest) to break auth ↔ api circular dependency.
// The refresh endpoint only needs the refresh token in the body — no JWT auth header.
async function doRefresh(): Promise<boolean> {
  const tokens = await getTokens();
  if (!tokens) return false;

  const settings = await getSettings();

  let outcome: RefreshOutcome;
  let response: Response | null = null;

  try {
    response = await fetch(`${settings.backendUrl}/api/v1/auth/extension-refresh`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ refresh_token: tokens.refresh_token }),
    });
    outcome = classifyRefreshStatus(response.status);
  } catch {
    // Network error (offline, DNS, TLS, abort). Treat as transient.
    outcome = "transient";
  }

  if (outcome === "ok" && response) {
    const raw = await response.json().catch(() => ({}));
    const parsed = RefreshResponseSchema.safeParse(raw);
    if (!parsed.success) {
      // Server returned 200 but a body we can't parse — treat as definitive
      // (something's structurally wrong, not transient).
      await clearTokens();
      await setSessionState("session_lost");
      consecutiveTransientFailures = 0;
      return false;
    }
    await storeTokens(parsed.data.tokens);
    await setSessionState("ok");
    consecutiveTransientFailures = 0;
    scheduleTokenRefresh(parsed.data.tokens.expires_in);
    return true;
  }

  if (outcome === "transient") {
    consecutiveTransientFailures += 1;
    const idx = consecutiveTransientFailures - 1;
    if (idx < REFRESH_BACKOFFS_MS.length) {
      // Still have retries left. Keep tokens; schedule another refresh.
      await setSessionState("refresh_retrying");
      scheduleRawRefresh(REFRESH_BACKOFFS_MS[idx]!);
      return false;
    }
    // Out of retries — give up and require re-pair.
    await clearTokens();
    await setSessionState("session_lost");
    consecutiveTransientFailures = 0;
    return false;
  }

  // outcome === "definitive": refresh token revoked/bad, or 4xx malformed request.
  await clearTokens();
  await setSessionState("session_lost");
  consecutiveTransientFailures = 0;
  return false;
}

// --- Scheduled Refresh ---
//
// MV3 background workers (Chrome SW, Firefox event page) suspend when idle,
// taking setTimeout with them. Alarms persist across restarts and wake the
// worker when they fire — this is the only timer that survives suspension.

const REFRESH_ALARM = "testudo-refresh";

function scheduleAlarm(delayMs: number): void {
  // Browser alarms enforce a minimum delay (~1 min on some Firefox versions);
  // values below the minimum are silently rounded up. Acceptable for our
  // 30s/2min/8min backoffs and 12-min refresh cadence.
  const delayInMinutes = Math.max(0.5, delayMs / 60000);
  browser.alarms.create(REFRESH_ALARM, { delayInMinutes });
}

export function scheduleTokenRefresh(expiresIn: number): void {
  scheduleAlarm(calculateRefreshDelay(expiresIn));
}

// Schedule a raw refresh after `delayMs` — used by the transient-failure
// backoff path to retry without recomputing from `expires_in`.
function scheduleRawRefresh(delayMs: number): void {
  scheduleAlarm(delayMs);
}

export function clearRefreshTimer(): void {
  browser.alarms.clear(REFRESH_ALARM);
  consecutiveTransientFailures = 0;
}

// Top-level alarm listener — must be registered synchronously at module init
// so the worker re-registers it on every cold start.
browser.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === REFRESH_ALARM) {
    refreshAccessToken();
  }
});

// --- Auth Status ---

export async function getAuthStatus(): Promise<{
  authenticated: boolean;
  walletAddress?: string;
  sessionState: SessionState;
}> {
  const sessionState = await getSessionState();
  const tokens = await getTokens();
  if (!tokens || tokens.expires_in <= 0) {
    return { authenticated: false, sessionState };
  }
  try {
    const payloadRaw = JSON.parse(atob(tokens.access_token.split(".")[1] || ""));
    const payload = JwtWalletPayloadSchema.safeParse(payloadRaw);
    if (!payload.success) {
      return { authenticated: true, sessionState };
    }
    return { authenticated: true, walletAddress: payload.data.wallet_address, sessionState };
  } catch {
    return { authenticated: true, sessionState };
  }
}
