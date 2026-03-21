import { createSignal, onMount, onCleanup, Show } from "solid-js";
import browser from "webextension-polyfill";
import ExchangeToggle from "./ExchangeToggle";
import ExchangeSelector from "./ExchangeSelector";

type SidecarStatus = "unknown" | "healthy" | "unreachable";
type ThemeKey = "amoled" | "light";

interface HeaderBarProps {
  onOpenSettings: () => void;
}

export default function HeaderBar(props: HeaderBarProps) {
  const [sidecarStatus, setSidecarStatus] = createSignal<SidecarStatus>("unknown");
  const [theme, setTheme] = createSignal<ThemeKey>("amoled");

  function handleMessage(message: unknown) {
    const msg = message as { type: string; status?: SidecarStatus };
    if (msg.type === "SIDECAR_STATUS_CHANGED" && msg.status) {
      setSidecarStatus(msg.status);
    }
  }

  onMount(async () => {
    const sidecarRes = await browser.runtime.sendMessage({ type: "SIDECAR_STATUS" }) as { status: SidecarStatus };
    setSidecarStatus(sidecarRes?.status || "unknown");
    browser.runtime.onMessage.addListener(handleMessage);

    const stored = await browser.storage.local.get("testudo-theme");
    const t = stored["testudo-theme"] as ThemeKey | undefined;
    if (t) setTheme(t);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

  function toggleTheme() {
    const next = theme() === "amoled" ? "light" : "amoled";
    setTheme(next);
    browser.storage.local.set({ "testudo-theme": next });
    localStorage.setItem("testudo-theme", next);
    if (next === "amoled") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", next);
    }
  }

  return (
    <>
      <div data-testid="header-bar" class="flex items-center justify-between px-4 h-11 border-b border-border-subtle">
        {/* Left: wordmark + theme toggle */}
        <div class="flex items-center gap-3">
          <span class="font-mono text-[13px] font-bold tracking-[0.15em] text-text-primary">
            TESTUDO
          </span>
          <button
            class="text-text-secondary hover:text-text-primary transition-colors border-0 bg-transparent cursor-pointer p-0"
            onClick={toggleTheme}
            title={`Theme: ${theme() === "amoled" ? "Dark" : "Light"}`}
            aria-label="Toggle theme"
          >
            {theme() === "amoled" ? (
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="5" />
                <line x1="12" y1="1" x2="12" y2="3" />
                <line x1="12" y1="21" x2="12" y2="23" />
                <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
                <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
                <line x1="1" y1="12" x2="3" y2="12" />
                <line x1="21" y1="12" x2="23" y2="12" />
                <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
                <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
              </svg>
            ) : (
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
              </svg>
            )}
          </button>
        </div>

        {/* Right: controls + settings */}
        <div class="flex items-center gap-1.5">
          <ExchangeToggle />
          <ExchangeSelector />
          <button
            class="flex items-center justify-center w-8 h-8 border border-border-subtle text-text-primary hover:bg-bg-elevated transition-colors cursor-pointer bg-transparent"
            onClick={props.onOpenSettings}
            data-testid="settings-btn"
            title="Settings"
          >
            <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>
        </div>
      </div>

      <Show when={sidecarStatus() === "unreachable"}>
        <div
          role="alert"
          class="mx-4 mb-2 mt-1 px-3 py-2 text-[11px] font-sans font-medium text-signal-orange bg-signal-orange/10 border border-signal-orange/20"
          data-testid="sidecar-warning-banner"
        >
          Live trading unavailable — exchange connection lost
        </div>
      </Show>
    </>
  );
}
