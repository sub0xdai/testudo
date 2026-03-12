import { createSignal, onMount, onCleanup } from "solid-js";
import browser from "webextension-polyfill";
import type { WsState } from "../../types";

type SidecarStatus = "unknown" | "healthy" | "unreachable";

const WS_STATE_LABELS: Record<WsState, string> = {
  disconnected: "Disconnected",
  connecting: "Connecting...",
  connected: "Connected",
};

const WS_DOT_CLASSES: Record<WsState, string> = {
  disconnected: "bg-signal-red",
  connecting: "bg-signal-orange status-blink",
  connected: "bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]",
};

export default function StatusBar() {
  const [wsState, setWsState] = createSignal<WsState>("disconnected");
  const [sidecarStatus, setSidecarStatus] = createSignal<SidecarStatus>("unknown");

  function handleMessage(message: unknown) {
    const msg = message as { type: string; state?: WsState; status?: SidecarStatus };
    if (msg.type === "WS_STATE_CHANGED" && msg.state) {
      setWsState(msg.state);
    }
    if (msg.type === "SIDECAR_STATUS_CHANGED" && msg.status) {
      setSidecarStatus(msg.status);
    }
  }

  onMount(async () => {
    // Register listener BEFORE querying so we don't miss state changes
    // that fire between the response and listener registration.
    browser.runtime.onMessage.addListener(handleMessage);

    const [wsRes, sidecarRes] = await Promise.all([
      browser.runtime.sendMessage({ type: "WS_STATUS" }) as Promise<{ state: WsState }>,
      browser.runtime.sendMessage({ type: "SIDECAR_STATUS" }) as Promise<{ status: SidecarStatus }>,
    ]);
    setWsState(wsRes.state);
    setSidecarStatus(sidecarRes?.status || "unknown");
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

  // Sidecar health is the primary exchange connectivity indicator.
  // WS state is supplementary (real-time order streaming only).
  const sidecarHealthy = () => sidecarStatus() === "healthy";
  const sidecarDown = () => sidecarStatus() === "unreachable";

  const dotClass = () => {
    if (sidecarHealthy()) return WS_DOT_CLASSES["connected"];
    if (sidecarDown()) return WS_DOT_CLASSES["disconnected"];
    return WS_DOT_CLASSES[wsState()];
  };

  const statusText = () => {
    if (sidecarHealthy()) return "Connected";
    if (sidecarDown()) return "Disconnected";
    return WS_STATE_LABELS[wsState()];
  };

  return (
    <div class="flex items-center gap-2" data-testid="status-bar">
      <span
        class={`w-2 h-2 rounded-full inline-block ${dotClass()}`}
        data-testid="status-dot"
        data-state={sidecarHealthy() ? "connected" : sidecarDown() ? "disconnected" : wsState()}
      />
      <span class="text-[12px] text-text-secondary font-sans font-medium" data-testid="status-text">
        {statusText()}
      </span>
    </div>
  );
}
