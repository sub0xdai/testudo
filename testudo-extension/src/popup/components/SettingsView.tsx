import { createSignal, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";

export default function SettingsView(props: { onBack: () => void; onLogout: () => void }) {
  const auth = useAuth();
  const [backendUrl, setBackendUrl] = createSignal("http://localhost:8080");
  const [wsUrl, setWsUrl] = createSignal("ws://localhost:4000");
  const [saved, setSaved] = createSignal("");

  onMount(async () => {
    const stored = await browser.storage.local.get(["backendUrl", "wsUrl"]);
    if (stored.backendUrl) setBackendUrl(stored.backendUrl as string);
    if (stored.wsUrl) setWsUrl(stored.wsUrl as string);
  });

  function showSaved(field: string) {
    setSaved(field);
    setTimeout(() => setSaved(""), 1500);
  }

  async function handleBackendChange(e: Event) {
    const value = (e.target as HTMLInputElement).value.trim();
    setBackendUrl(value);
    await browser.storage.local.set({ backendUrl: value });
    showSaved("backend");
  }

  async function handleWsChange(e: Event) {
    const value = (e.target as HTMLInputElement).value.trim();
    setWsUrl(value);
    await browser.storage.local.set({ wsUrl: value });
    showSaved("ws");
  }

  async function handleLogout() {
    await auth.logout();
    props.onLogout();
  }

  return (
    <div class="flex flex-col min-h-full">
      {/* Header */}
      <div class="flex items-center gap-3 px-4 py-3 border-b-2 border-border-grid">
        <button
          class="p-1 border-0 text-text-secondary hover:text-text-primary hover:bg-transparent"
          onClick={props.onBack}
          data-testid="settings-back"
          title="Back"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>
        <h1 class="text-sm font-display font-bold tracking-[0.2em] text-text-primary">
          SETTINGS
        </h1>
      </div>

      {/* Body */}
      <div class="flex-1 px-4 py-4 space-y-5">
        {/* Backend URL */}
        <div>
          <label class="block text-[11px] text-text-dim font-display uppercase tracking-widest mb-2">
            Backend URL
          </label>
          <input
            type="url"
            value={backendUrl()}
            onChange={handleBackendChange}
            placeholder="http://localhost:8080"
            data-testid="backend-url"
          />
          <Show when={saved() === "backend"}>
            <span class="text-[10px] text-signal-green mt-1 block" data-testid="save-status">SAVED</span>
          </Show>
        </div>

        {/* WebSocket URL */}
        <div>
          <label class="block text-[11px] text-text-dim font-display uppercase tracking-widest mb-2">
            WebSocket URL
          </label>
          <input
            type="url"
            value={wsUrl()}
            onChange={handleWsChange}
            placeholder="ws://localhost:4000"
            data-testid="ws-url"
          />
          <Show when={saved() === "ws"}>
            <span class="text-[10px] text-signal-green mt-1 block" data-testid="save-status">SAVED</span>
          </Show>
        </div>

        {/* Account Section */}
        <div class="pt-4 border-t-2 border-border-grid">
          <label class="block text-[11px] text-text-dim font-display uppercase tracking-widest mb-3">
            Account
          </label>
          <Show
            when={auth.authenticated()}
            fallback={
              <p class="text-xs text-text-dim font-mono">Paper mode — no account</p>
            }
          >
            <div class="space-y-3">
              <p class="text-xs font-mono text-text-secondary" data-testid="settings-email">
                {auth.email()}
              </p>
              <button
                class="w-full py-2 text-xs font-bold tracking-widest border-signal-red text-signal-red hover:bg-signal-red hover:text-text-primary"
                onClick={handleLogout}
                data-testid="logout-btn"
              >
                LOGOUT
              </button>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}
