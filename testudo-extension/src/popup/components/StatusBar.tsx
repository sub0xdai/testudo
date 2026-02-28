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
  connected: "bg-accent-steel shadow-[0_0_8px_rgba(148,163,184,0.6)]",
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
    const [wsRes, sidecarRes] = await Promise.all([
      browser.runtime.sendMessage({ type: "WS_STATUS" }) as Promise<{ state: WsState }>,
      browser.runtime.sendMessage({ type: "SIDECAR_STATUS" }) as Promise<{ status: SidecarStatus }>,
    ]);
    setWsState(wsRes.state);
    setSidecarStatus(sidecarRes?.status || "unknown");
    browser.runtime.onMessage.addListener(handleMessage);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

  // Sidecar unreachable overrides to orange warning
  const sidecarDown = () => sidecarStatus() === "unreachable";

  const dotClass = () => {
    if (sidecarDown()) return "bg-signal-orange status-blink";
    return WS_DOT_CLASSES[wsState()];
  };

  const statusText = () => {
    if (sidecarDown()) return "Sidecar Down";
    return WS_STATE_LABELS[wsState()];
  };

  return (
    <div class="flex items-center gap-2" data-testid="status-bar">
      <span
        class={`w-2 h-2 rounded-full inline-block ${dotClass()}`}
        data-testid="status-dot"
        data-state={sidecarDown() ? "sidecar-down" : wsState()}
      />
      <span class="text-[12px] text-text-secondary font-sans font-medium" data-testid="status-text">
        {statusText()}
      </span>
    </div>
  );
}
