import { Show } from "solid-js";
import { useAuth } from "../context/AuthContext";
import TradeManagement from "./TradeManagement";
import ActiveOrders from "./ActiveOrders";
import ModeToggle from "./ModeToggle";
import StatusBar from "./StatusBar";

export default function MainView(props: { onOpenSettings: () => void }) {
  const auth = useAuth();

  return (
    <div class="flex flex-col min-h-full">
      {/* Header */}
      <div class="flex items-center justify-between px-4 py-3 border-b-2 border-border-grid">
        <h1 class="text-base font-display font-bold tracking-[0.2em] text-text-primary">
          TESTUDO
        </h1>
        <button
          class="p-1 border-0 text-text-secondary hover:text-text-primary hover:bg-transparent"
          onClick={props.onOpenSettings}
          data-testid="settings-btn"
          title="Settings"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </button>
      </div>

      {/* Body */}
      <div class="flex-1 px-4 py-3 space-y-4">
        <TradeManagement />
        <div class="border-t-2 border-border-grid pt-3">
          <ActiveOrders />
        </div>
        <ModeToggle />
      </div>

      {/* Footer */}
      <div class="px-4 py-2 border-t-2 border-border-grid flex items-center justify-between">
        <StatusBar />
        <Show when={auth.email()}>
          <span class="text-[11px] font-mono text-text-dim truncate max-w-[140px]" data-testid="footer-email">
            {auth.email()}
          </span>
        </Show>
        <Show when={auth.paperOnly()}>
          <span class="text-[11px] font-mono text-text-dim" data-testid="footer-paper">
            PAPER ONLY
          </span>
        </Show>
      </div>
    </div>
  );
}
