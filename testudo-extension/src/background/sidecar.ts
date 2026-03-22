import browser from "webextension-polyfill";
import { SidecarHealthResponseSchema } from "../schemas";
import { apiRequest } from "./api";

// --- Sidecar Health Polling (EXT-16 FR-2) ---

export type SidecarStatus = "unknown" | "healthy" | "unreachable";
let sidecarStatus: SidecarStatus = "unknown";
let sidecarHealthTimer: ReturnType<typeof setInterval> | null = null;
let sidecarHealthInitialTimer: ReturnType<typeof setTimeout> | null = null;

export function getSidecarStatus(): SidecarStatus {
  return sidecarStatus;
}

export function setSidecarStatus(status: SidecarStatus): void {
  if (status === sidecarStatus) return;
  sidecarStatus = status;
  browser.runtime.sendMessage({ type: "SIDECAR_STATUS_CHANGED", status }).catch(() => {});
}

export async function checkSidecarHealth(): Promise<void> {
  const result = await apiRequest("/api/v1/health/sidecar", { timeout: 5000 });
  if (!result.ok) { setSidecarStatus("unreachable"); return; }
  const json = SidecarHealthResponseSchema.safeParse(result.raw);
  setSidecarStatus(json.success && json.data.status === "healthy" ? "healthy" : "unreachable");
}

export function startSidecarHealthPolling(): void {
  if (sidecarHealthTimer) return;
  sidecarHealthInitialTimer = setTimeout(checkSidecarHealth, 5000);
  sidecarHealthTimer = setInterval(checkSidecarHealth, 30000);
}

export function stopSidecarHealthPolling(): void {
  if (sidecarHealthInitialTimer) {
    clearTimeout(sidecarHealthInitialTimer);
    sidecarHealthInitialTimer = null;
  }
  if (sidecarHealthTimer) {
    clearInterval(sidecarHealthTimer);
    sidecarHealthTimer = null;
  }
}
