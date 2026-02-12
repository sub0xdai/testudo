import { createSignal, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";

type ExecutionMode = "paper" | "live";

export default function ModeToggle(props: { compact?: boolean }) {
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

  if (props.compact) {
    return (
      <div class="flex border border-border-grid" data-testid="mode-toggle">
        <button
          class={`px-3 py-0.5 text-[10px] font-bold tracking-wider border-0 font-sans ${
            mode() === "paper"
              ? "bg-signal-green/15 text-signal-green"
              : "text-text-dim"
          }`}
          onClick={() => selectMode("paper")}
          data-testid="mode-paper"
          data-mode="paper"
        >
          PAPER
        </button>
        <Show when={!auth.paperOnly()}>
          <button
            class={`px-3 py-0.5 text-[10px] font-bold tracking-wider border-0 border-l border-l-border-grid font-sans ${
              mode() === "live"
                ? "bg-signal-red/15 text-signal-red"
                : "text-text-dim"
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

  return (
    <div class="flex gap-2" data-testid="mode-toggle">
      <button
        class={`flex-1 py-2 text-xs font-bold tracking-widest font-sans ${
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
          class={`flex-1 py-2 text-xs font-bold tracking-widest font-sans ${
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
