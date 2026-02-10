import { createSignal, onMount, onCleanup } from "solid-js";
import browser from "webextension-polyfill";
import type { WsState } from "../../types";

const WS_STATE_LABELS: Record<WsState, string> = {
  disconnected: "Disconnected",
  connecting: "Connecting...",
  connected: "Connected",
};

const WS_DOT_CLASSES: Record<WsState, string> = {
  disconnected: "bg-red-500",
  connecting: "bg-yellow-400 animate-pulse",
  connected: "bg-emerald-400",
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
    <div class="flex items-center gap-1.5 text-xs text-zinc-500 pt-3 mt-3 border-t border-zinc-700" data-testid="status-bar">
      <span
        class={`w-2 h-2 rounded-full ${WS_DOT_CLASSES[wsState()]}`}
        data-testid="status-dot"
        data-state={wsState()}
      />
      <span data-testid="status-text">{WS_STATE_LABELS[wsState()]}</span>
    </div>
  );
}
