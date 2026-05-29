/** @anchor ui:ext-popup:HeaderBar
 * @tags ui */

import { createSignal, onMount, onCleanup, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";
import ExchangeToggle from "./ExchangeToggle";
import ExchangeSelector from "./ExchangeSelector";
import { DESK_URL } from "../../utils";

type SidecarStatus = "unknown" | "healthy" | "unreachable";
type ThemeKey = "amoled" | "light";

interface HeaderBarProps {
  onLogout: () => void;
}

export default function HeaderBar(props: HeaderBarProps) {
  const auth = useAuth();
  const [sidecarStatus, setSidecarStatus] = createSignal<SidecarStatus>("unknown");
  const [theme, setTheme] = createSignal<ThemeKey>("amoled");
  const [menuOpen, setMenuOpen] = createSignal(false);

  function handleMessage(message: unknown) {
    const msg = message as { type: string; status?: SidecarStatus };
    if (msg.type === "SIDECAR_STATUS_CHANGED" && msg.status) {
      setSidecarStatus(msg.status);
    }
  }

  function handleClickOutside(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest("[data-menu-container]")) {
      setMenuOpen(false);
    }
  }

  onMount(async () => {
    const sidecarRes = await browser.runtime.sendMessage({ type: "SIDECAR_STATUS" }) as { status: SidecarStatus };
    setSidecarStatus(sidecarRes?.status || "unknown");
    browser.runtime.onMessage.addListener(handleMessage);
    document.addEventListener("click", handleClickOutside);

    const stored = await browser.storage.local.get("testudo-theme");
    const t = stored["testudo-theme"] as ThemeKey | undefined;
    if (t) setTheme(t);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
    document.removeEventListener("click", handleClickOutside);
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

  async function handleLogout() {
    setMenuOpen(false);
    await auth.logout();
    props.onLogout();
  }

  return (
    <>
      <div data-testid="header-bar" class="flex items-center justify-between px-4 h-11 border-b border-border-subtle">
        {/* Left: wordmark + theme toggle */}
        <div class="flex items-center gap-3">
          <img src="popup/images/shield.svg" alt="" class="crest-logo w-4 h-4 object-contain opacity-60" />
          <span class="font-mono text-[13px] font-bold tracking-[0.15em] text-text-primary">
            TESTUDO
          </span>
          <button
            class="btn-ghost p-0 transition-colors cursor-pointer"
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

        {/* Right: controls + menu */}
        <div class="flex items-center gap-1.5">
          <ExchangeToggle />
          <ExchangeSelector />
          <div class="relative" data-menu-container>
            <button
              class="flex items-center justify-center w-8 h-8 p-0 border border-border-subtle text-text-primary hover:text-text-primary hover:bg-bg-elevated transition-colors cursor-pointer bg-transparent"
              onClick={() => setMenuOpen(!menuOpen())}
              data-testid="menu-btn"
              title="Menu"
              aria-haspopup="true"
              aria-expanded={menuOpen()}
            >
              <svg aria-hidden="true" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <line x1="3" y1="6" x2="21" y2="6" />
                <line x1="3" y1="12" x2="21" y2="12" />
                <line x1="3" y1="18" x2="21" y2="18" />
              </svg>
            </button>

            <Show when={menuOpen()}>
              <div
                class="absolute right-0 top-full mt-1 w-48 bg-bg-elevated border border-border-subtle shadow-lg z-50"
                role="menu"
                data-testid="header-menu"
              >
                <Show
                  when={auth.authenticated()}
                  fallback={
                    <button
                      role="menuitem"
                      class="btn-ghost w-full px-4 py-2.5 text-left text-[13px] font-sans hover:bg-bg-panel transition-colors cursor-pointer"
                      onClick={() => { setMenuOpen(false); props.onLogout(); }}
                      data-testid="menu-sign-in"
                    >
                      Sign In
                    </button>
                  }
                >
                  <div class="px-4 py-2.5 text-[11px] font-mono text-text-dim truncate border-b border-border-subtle">
                    {auth.walletAddress()}
                  </div>
                  <button
                    role="menuitem"
                    class="btn-ghost w-full px-4 py-2.5 text-left text-[13px] font-sans hover:bg-bg-panel transition-colors cursor-pointer"
                    onClick={() => { setMenuOpen(false); window.open(DESK_URL, "_blank"); }}
                    data-testid="menu-trading-desk"
                  >
                    Trading Desk
                  </button>
                  <button
                    role="menuitem"
                    class="btn-ghost w-full px-4 py-2.5 text-left text-[13px] font-sans hover:bg-bg-panel transition-colors cursor-pointer"
                    onClick={() => { setMenuOpen(false); window.open(`${DESK_URL}/account?source=extension`, "_blank"); }}
                    data-testid="menu-manage-account"
                  >
                    Manage Account
                  </button>
                  <div class="border-t border-border-subtle" />
                  <button
                    role="menuitem"
                    class="btn-ghost w-full px-4 py-2.5 text-left text-[13px] font-sans text-signal-red hover:bg-bg-panel transition-colors cursor-pointer"
                    onClick={handleLogout}
                    data-testid="menu-logout"
                  >
                    Log Out
                  </button>
                </Show>
              </div>
            </Show>
          </div>
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
