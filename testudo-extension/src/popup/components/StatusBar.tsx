import { createSignal, onMount, onCleanup, Show } from "solid-js";
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
  connected: "bg-accent-aqua shadow-[0_0_8px_rgba(142,192,124,0.6)]",
};

export default function StatusBar() {
  const [wsState, setWsState] = createSignal<WsState>("disconnected");
  const [sidecarStatus, setSidecarStatus] = createSignal<SidecarStatus>("unknown");
  const [executionMode, setExecutionMode] = createSignal<string>("paper");

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
    const [wsRes, sidecarRes, stored] = await Promise.all([
      browser.runtime.sendMessage({ type: "WS_STATUS" }) as Promise<{ state: WsState }>,
      browser.runtime.sendMessage({ type: "SIDECAR_STATUS" }) as Promise<{ status: SidecarStatus }>,
      browser.storage.local.get(["executionMode"]),
    ]);
    setWsState(wsRes.state);
    setSidecarStatus(sidecarRes?.status || "unknown");
    setExecutionMode((stored.executionMode as string) || "paper");
    browser.runtime.onMessage.addListener(handleMessage);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

  // Listen for execution mode changes
  const storageListener = (changes: Record<string, browser.Storage.StorageChange>) => {
    if (changes.executionMode) {
      setExecutionMode(changes.executionMode.newValue as string);
    }
  };
  onMount(() => browser.storage.onChanged.addListener(storageListener));
  onCleanup(() => browser.storage.onChanged.removeListener(storageListener));

  // Compound status: in LIVE mode, sidecar unreachable overrides to orange
  const isLive = () => executionMode() === "live";
  const sidecarDown = () => isLive() && sidecarStatus() === "unreachable";

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
