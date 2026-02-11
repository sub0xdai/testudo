import { createSignal, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";

type ExecutionMode = "paper" | "live";

export default function ModeToggle() {
  const auth = useAuth();
  const [mode, setMode] = createSignal<ExecutionMode>("paper");

  onMount(async () => {
    const stored = await browser.storage.local.get(["executionMode"]);
    if (stored.executionMode) setMode(stored.executionMode as ExecutionMode);
  });

  async function selectMode(m: ExecutionMode) {
    setMode(m);
    await browser.storage.local.set({ executionMode: m });
  }

  return (
    <div class="flex gap-2" data-testid="mode-toggle">
      <button
        class={`flex-1 py-2 text-xs font-bold tracking-widest ${
          mode() === "paper"
            ? "bg-signal-green text-bg-core border-signal-green"
            : "text-text-secondary border-border-grid hover:bg-bg-elevated"
        }`}
        onClick={() => selectMode("paper")}
        data-testid="mode-paper"
        data-mode="paper"
      >
        PAPER
      </button>
      <Show when={!auth.paperOnly()}>
        <button
          class={`flex-1 py-2 text-xs font-bold tracking-widest ${
            mode() === "live"
              ? "bg-signal-red text-text-primary border-signal-red"
              : "text-text-secondary border-border-grid hover:bg-bg-elevated"
          }`}
          onClick={() => selectMode("live")}
          data-testid="mode-live"
          data-mode="live"
        >
          LIVE
        </button>
      </Show>
    </div>
  );
}
