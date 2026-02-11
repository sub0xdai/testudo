import { createSignal, onMount, onCleanup } from "solid-js";
import browser from "webextension-polyfill";
import type { WsState } from "../../types";

const WS_STATE_LABELS: Record<WsState, string> = {
  disconnected: "Disconnected",
  connecting: "Connecting...",
  connected: "Connected",
};

const WS_DOT_CLASSES: Record<WsState, string> = {
  disconnected: "bg-signal-red",
  connecting: "bg-signal-orange status-blink",
  connected: "bg-signal-green",
};

export default function StatusBar() {
  const [wsState, setWsState] = createSignal<WsState>("disconnected");

  function handleMessage(message: unknown) {
    const msg = message as { type: string; state?: WsState };
    if (msg.type === "WS_STATE_CHANGED" && msg.state) {
      setWsState(msg.state);
    }
  }

  onMount(async () => {
    const response = await browser.runtime.sendMessage({ type: "WS_STATUS" }) as { state: WsState };
    setWsState(response.state);
    browser.runtime.onMessage.addListener(handleMessage);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

  return (
    <div class="flex items-center gap-1.5" data-testid="status-bar">
      <span
        class={`w-2 h-2 inline-block ${WS_DOT_CLASSES[wsState()]}`}
        data-testid="status-dot"
        data-state={wsState()}
      />
      <span class="text-[11px] text-text-dim font-mono" data-testid="status-text">
        {WS_STATE_LABELS[wsState()]}
      </span>
    </div>
  );
}
