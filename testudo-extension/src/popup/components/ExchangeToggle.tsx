import { createSignal, onMount, onCleanup, For } from "solid-js";
import browser from "webextension-polyfill";
import type { ExchangeMode } from "../../utils";

const MODES: { value: ExchangeMode; label: string }[] = [
  { value: "cex", label: "CEX" },
  { value: "dex", label: "DEX" },
];

export default function ExchangeToggle() {
  const [mode, setMode] = createSignal<ExchangeMode>("cex");

  async function fetchMode() {
    try {
      const res = await browser.runtime.sendMessage({ type: "GET_EXCHANGE_MODE" }) as { mode: ExchangeMode };
      if (res?.mode) setMode(res.mode);
    } catch { /* non-blocking */ }
  }

  async function switchMode(newMode: ExchangeMode) {
    if (newMode === mode()) return;
    setMode(newMode);
    await browser.runtime.sendMessage({ type: "SET_EXCHANGE_MODE", mode: newMode });
  }

  function handleStorageChange(changes: Record<string, { oldValue?: unknown; newValue?: unknown }>) {
    if (changes.exchangeMode) {
      setMode((changes.exchangeMode.newValue as ExchangeMode) || "cex");
    }
  }

  onMount(() => {
    fetchMode();
    browser.storage.onChanged.addListener(handleStorageChange);
  });

  onCleanup(() => {
    browser.storage.onChanged.removeListener(handleStorageChange);
  });

  return (
    <div
      class="flex items-center bg-bg-panel p-[3px] gap-0"
      data-testid="exchange-toggle"
      role="radiogroup"
      aria-label="Exchange mode"
    >
      <For each={MODES}>
        {(m) => (
          <button
            role="radio"
            aria-checked={mode() === m.value}
            class={`px-2.5 py-1 text-[11px] font-bold tracking-wider border-0 transition-colors cursor-pointer ${
              mode() === m.value
                ? "bg-text-primary/10 text-text-primary"
                : "text-text-dim hover:text-text-secondary bg-transparent"
            }`}
            onClick={() => switchMode(m.value)}
          >
            {m.label}
          </button>
        )}
      </For>
    </div>
  );
}
