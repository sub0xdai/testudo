/** @anchor ui:ext-popup:StatusBar
 * @tags ui */

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
  connected: "bg-signal-green shadow-[0_0_8px_rgba(34,197,94,0.6)]",
};

interface StatusBarProps {
  sidecarStatus: SidecarStatus;
}

export default function StatusBar(props: StatusBarProps) {
  const [wsState, setWsState] = createSignal<WsState>("disconnected");

  function handleMessage(message: unknown) {
    const msg = message as { type: string; state?: WsState };
    if (msg.type === "WS_STATE_CHANGED" && msg.state) {
      setWsState(msg.state);
    }
  }

  onMount(async () => {
    browser.runtime.onMessage.addListener(handleMessage);
    const wsRes = await browser.runtime.sendMessage({ type: "WS_STATUS" }) as { state: WsState };
    setWsState(wsRes.state);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

  const sidecarHealthy = () => props.sidecarStatus === "healthy";
  const sidecarDown = () => props.sidecarStatus === "unreachable";

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
