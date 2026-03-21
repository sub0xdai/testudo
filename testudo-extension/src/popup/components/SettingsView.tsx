import { createSignal, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";

export default function SettingsView(props: { onBack: () => void; onLogout: () => void }) {
  const auth = useAuth();
  const [webUrl, setWebUrl] = createSignal("http://localhost:3001");

  onMount(async () => {
    const stored = await browser.storage.local.get(["webUrl"]);
    if (stored.webUrl) setWebUrl(stored.webUrl as string);
  });

  async function handleLogout() {
    await auth.logout();
    props.onLogout();
  }

  return (
    <div class="flex flex-col h-full">
      {/* Header */}
      <div class="flex items-center gap-4 px-5 py-3.5">
        <button
          class="icon-btn border-0 text-text-dim hover:text-text-primary hover:bg-bg-elevated"
          onClick={props.onBack}
          data-testid="settings-back"
          title="Back"
        >
          <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
        </button>
        <span class="text-[14px] font-sans font-bold tracking-[0.1em] text-text-primary">
          Settings
        </span>
      </div>

      {/* Body */}
      <div class="flex-1 px-5 py-4 scroll-area">

        {/* ── ACCOUNT ── */}
        <div class="text-[11px] text-text-dim font-sans font-medium tracking-widest mb-3">
          ACCOUNT
        </div>

        <Show
          when={auth.authenticated()}
          fallback={
            <div class="mb-6">
              <p class="text-xs text-text-dim font-sans mb-3">Paper mode — no account</p>
              <button
                class="w-full py-2.5 text-xs font-bold tracking-widest font-sans border border-text-primary/30 text-text-primary hover:bg-text-primary hover:text-bg-core transition-colors"
                onClick={handleLogout}
                data-testid="sign-in-btn"
              >
                SIGN IN
              </button>
            </div>
          }
        >
          <div class="mb-6">
            <p class="text-[13px] font-mono text-text-primary mb-4" data-testid="settings-email">
              {auth.email()}
            </p>

            <div class="flex gap-2">
              <button
                class="flex-1 py-2.5 text-xs font-bold tracking-widest font-sans border border-text-primary/30 text-text-primary hover:bg-text-primary hover:text-bg-core transition-colors"
                onClick={() => {
                  const base = webUrl().replace(/\/$/, '');
                  window.open(`${base}/account`, '_blank');
                }}
                data-testid="manage-accounts-btn"
              >
                MANAGE EXCHANGES
              </button>
              <button
                class="flex-1 py-2.5 text-xs font-bold tracking-widest font-sans border border-signal-red/30 text-signal-red hover:bg-signal-red hover:text-text-primary transition-colors"
                onClick={handleLogout}
                data-testid="logout-btn"
              >
                LOGOUT
              </button>
            </div>
          </div>
        </Show>


      </div>
    </div>
  );
}
