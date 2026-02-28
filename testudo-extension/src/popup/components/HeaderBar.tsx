import { createSignal, onMount, onCleanup, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";
import ModeToggle from "./ModeToggle";
import StatusBar from "./StatusBar";
import ExchangeSelector from "./ExchangeSelector";

type SidecarStatus = "unknown" | "healthy" | "unreachable";

interface HeaderBarProps {
  onOpenSettings: () => void;
}

export default function HeaderBar(props: HeaderBarProps) {
  const auth = useAuth();
  const [sidecarStatus, setSidecarStatus] = createSignal<SidecarStatus>("unknown");
  const [executionMode, setExecutionMode] = createSignal<string>("paper");

  function handleMessage(message: unknown) {
    const msg = message as { type: string; status?: SidecarStatus };
    if (msg.type === "SIDECAR_STATUS_CHANGED" && msg.status) {
      setSidecarStatus(msg.status);
    }
  }

  onMount(async () => {
    const [sidecarRes, stored] = await Promise.all([
      browser.runtime.sendMessage({ type: "SIDECAR_STATUS" }) as Promise<{ status: SidecarStatus }>,
      browser.storage.local.get(["executionMode"]),
    ]);
    setSidecarStatus(sidecarRes?.status || "unknown");
    setExecutionMode((stored.executionMode as string) || "paper");
    browser.runtime.onMessage.addListener(handleMessage);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

  const storageListener = (changes: Record<string, browser.Storage.StorageChange>) => {
    if (changes.executionMode) {
      setExecutionMode(changes.executionMode.newValue as string);
    }
  };
  onMount(() => browser.storage.onChanged.addListener(storageListener));
  onCleanup(() => browser.storage.onChanged.removeListener(storageListener));

  const showBanner = () => executionMode() === "live" && sidecarStatus() === "unreachable";

  return (
    <>
      <div data-testid="header-bar" class="flex items-center justify-between px-5 py-2.5">
        <div class="flex items-center gap-2">
          <StatusBar />
        </div>
        <div class="flex items-center gap-2">
          <Show when={!auth.paperOnly()}>
            <ExchangeSelector />
          </Show>
          <ModeToggle compact />
          <button
            class="p-1.5 border-0 rounded-lg text-text-dim hover:text-text-primary hover:bg-bg-elevated"
            onClick={props.onOpenSettings}
            data-testid="settings-btn"
            title="Settings"
          >
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          </button>
        </div>
      </div>
      <Show when={showBanner()}>
        <div
          class="mx-5 mb-2 px-3 py-2 rounded-lg text-[11px] font-sans font-medium text-signal-orange bg-signal-orange/10 border border-signal-orange/20"
          data-testid="sidecar-warning-banner"
        >
          Live trading unavailable — exchange connection lost
        </div>
      </Show>
    </>
  );
}
