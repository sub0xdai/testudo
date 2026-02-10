import { createSignal, onMount } from "solid-js";
import browser from "webextension-polyfill";

type ExecutionMode = "paper" | "live";

export default function Settings() {
  const [backendUrl, setBackendUrl] = createSignal("http://localhost:8080");
  const [wsUrl, setWsUrl] = createSignal("ws://localhost:4000");
  const [mode, setMode] = createSignal<ExecutionMode>("paper");
  const [saved, setSaved] = createSignal(false);

  onMount(async () => {
    const stored = await browser.storage.local.get(["backendUrl", "wsUrl", "executionMode"]);
    if (stored.backendUrl) setBackendUrl(stored.backendUrl as string);
    if (stored.wsUrl) setWsUrl(stored.wsUrl as string);
    if (stored.executionMode) setMode(stored.executionMode as ExecutionMode);
  });

  async function save(overrides?: Record<string, string>) {
    await browser.storage.local.set({
      backendUrl: backendUrl(),
      wsUrl: wsUrl(),
      executionMode: mode(),
      ...overrides,
    });
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  }

  function handleBackendChange(e: Event) {
    const value = (e.target as HTMLInputElement).value.trim();
    setBackendUrl(value);
    save({ backendUrl: value });
  }

  function handleWsChange(e: Event) {
    const value = (e.target as HTMLInputElement).value.trim();
    setWsUrl(value);
    save({ wsUrl: value });
  }

  function selectMode(m: ExecutionMode) {
    setMode(m);
    save({ executionMode: m });
  }

  return (
    <div class="space-y-3 pb-3 mb-3 border-b border-zinc-700" data-testid="settings">
      <div>
        <label class="block text-[11px] text-zinc-500 uppercase tracking-wide mb-1">Backend URL</label>
        <input
          type="text"
          class="w-full px-2.5 py-2 bg-[#16213e] border border-zinc-700 text-zinc-200 font-mono text-sm focus:outline-none focus:border-emerald-400"
          value={backendUrl()}
          onChange={handleBackendChange}
          placeholder="http://localhost:8080"
          data-testid="backend-url"
        />
      </div>
      <div>
        <label class="block text-[11px] text-zinc-500 uppercase tracking-wide mb-1">WebSocket URL</label>
        <input
          type="text"
          class="w-full px-2.5 py-2 bg-[#16213e] border border-zinc-700 text-zinc-200 font-mono text-sm focus:outline-none focus:border-emerald-400"
          value={wsUrl()}
          onChange={handleWsChange}
          placeholder="ws://localhost:4000"
          data-testid="ws-url"
        />
      </div>
      <div>
        <label class="block text-[11px] text-zinc-500 uppercase tracking-wide mb-1">Execution Mode</label>
        <div class="flex">
          <button
            class={`flex-1 py-2 text-xs font-medium uppercase tracking-wide border border-zinc-700 border-r-0 ${
              mode() === "paper"
                ? "bg-emerald-400 text-[#1a1a2e] border-emerald-400"
                : "bg-[#16213e] text-zinc-500"
            }`}
            onClick={() => selectMode("paper")}
            data-testid="mode-paper"
            data-mode="paper"
          >
            Paper
          </button>
          <button
            class={`flex-1 py-2 text-xs font-medium uppercase tracking-wide border border-zinc-700 ${
              mode() === "live"
                ? "bg-red-500 text-white border-red-500"
                : "bg-[#16213e] text-zinc-500"
            }`}
            onClick={() => selectMode("live")}
            data-testid="mode-live"
            data-mode="live"
          >
            Live
          </button>
        </div>
      </div>
      <div
        class={`text-xs text-emerald-400 transition-opacity duration-300 ${saved() ? "opacity-100" : "opacity-0"}`}
        data-testid="save-status"
      >
        Settings saved
      </div>
    </div>
  );
}
