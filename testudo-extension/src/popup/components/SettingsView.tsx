import { createSignal, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";

export default function SettingsView(props: { onBack: () => void; onLogout: () => void }) {
  const auth = useAuth();
  const [backendUrl, setBackendUrl] = createSignal("http://localhost:8080");
  const [wsUrl, setWsUrl] = createSignal("ws://localhost:4000");
  const [webUrl, setWebUrl] = createSignal("http://localhost:3001");
  const [saved, setSaved] = createSignal("");

  onMount(async () => {
    const stored = await browser.storage.local.get(["backendUrl", "wsUrl", "webUrl"]);
    if (stored.backendUrl) setBackendUrl(stored.backendUrl as string);
    if (stored.wsUrl) setWsUrl(stored.wsUrl as string);
    if (stored.webUrl) setWebUrl(stored.webUrl as string);
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
    <div class="flex flex-col h-full">
      {/* Header */}
      <div class="flex items-center gap-3 px-5 py-3.5">
        <button
          class="p-1.5 border-0 rounded-lg text-text-dim hover:text-text-primary hover:bg-bg-elevated"
          onClick={props.onBack}
          data-testid="settings-back"
          title="Back"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>
        <span class="text-[14px] font-sans font-bold tracking-[0.1em] text-white">
          Settings
        </span>
      </div>

      {/* Body */}
      <div class="flex-1 px-5 py-4 space-y-5 scroll-area">
        {/* Backend URL */}
        <div>
          <label class="block text-[11px] text-text-secondary font-sans font-medium mb-2">
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
            <span class="text-[10px] text-signal-green font-sans mt-1.5 block" data-testid="save-status">Saved</span>
          </Show>
        </div>

        {/* WebSocket URL */}
        <div>
          <label class="block text-[11px] text-text-secondary font-sans font-medium mb-2">
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
            <span class="text-[10px] text-signal-green font-sans mt-1.5 block" data-testid="save-status">Saved</span>
          </Show>
        </div>

        {/* Web App URL */}
        <div>
          <label class="block text-[11px] text-text-secondary font-sans font-medium mb-2">
            Web App URL
          </label>
          <input
            type="url"
            value={webUrl()}
            onChange={async (e) => {
              const value = (e.target as HTMLInputElement).value.trim();
              setWebUrl(value);
              await browser.storage.local.set({ webUrl: value });
              showSaved("web");
            }}
            placeholder="http://localhost:3001"
            data-testid="web-url"
          />
          <Show when={saved() === "web"}>
            <span class="text-[10px] text-signal-green font-sans mt-1.5 block" data-testid="save-status">Saved</span>
          </Show>
        </div>

        {/* Account Section */}
        <div class="pt-4 border-t border-border-subtle">
          <label class="block text-[11px] text-text-secondary font-sans font-medium mb-3">
            Account
          </label>
          <Show
            when={auth.authenticated()}
            fallback={
              <div class="space-y-3">
                <p class="text-xs text-text-dim font-sans">Paper mode — no account</p>
                <button
                  class="w-full py-2.5 text-xs font-bold tracking-widest font-sans rounded-xl border-accent-steel/40 text-accent-steel hover:bg-accent-steel/10"
                  onClick={handleLogout}
                  data-testid="sign-in-btn"
                >
                  SIGN IN
                </button>
              </div>
            }
          >
            <div class="space-y-3">
              <p class="text-[13px] font-mono text-text-secondary" data-testid="settings-email">
                {auth.email()}
              </p>
              <button
                class="w-full py-2.5 text-xs font-bold tracking-widest font-sans rounded-xl border-signal-red text-signal-red hover:bg-signal-red hover:text-white"
                onClick={handleLogout}
                data-testid="logout-btn"
              >
                LOGOUT
              </button>
            </div>
          </Show>
        </div>

        {/* Exchange Accounts — managed on web */}
        <Show when={auth.authenticated()}>
          <div class="pt-4 border-t border-border-subtle">
            <label class="block text-[11px] text-text-secondary font-sans font-medium mb-3">
              Exchange Accounts
            </label>
            <button
              class="w-full py-2.5 text-xs font-bold tracking-widest font-sans rounded-xl border-accent-steel/40 text-accent-steel hover:bg-accent-steel/10"
              onClick={() => {
                const base = webUrl().replace(/\/$/, '');
                window.open(`${base}/account`, '_blank');
              }}
              data-testid="manage-accounts-btn"
            >
              MANAGE ACCOUNTS
            </button>
            <p class="text-[10px] text-text-dim font-sans mt-2 text-center">
              Opens account management in your browser
            </p>
          </div>
        </Show>
      </div>
    </div>
  );
}
