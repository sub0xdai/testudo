import { Show } from "solid-js";
import ModeToggle from "./ModeToggle";
import StatusBar from "./StatusBar";

interface HeaderBarProps {
  balance: number | null;
  balanceLoading: boolean;
  onOpenSettings: () => void;
}

function formatBalance(value: number): string {
  return value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

export default function HeaderBar(props: HeaderBarProps) {
  return (
    <div data-testid="header-bar">
      {/* Row 1: Logo + Status + Mode + Gear */}
      <div class="flex items-center justify-between px-4 py-2 border-b border-border-grid">
        <div class="flex items-center gap-2">
          <StatusBar />
          <h1 class="text-base font-display font-bold tracking-[0.2em] text-text-primary">
            TESTUDO
          </h1>
        </div>
        <div class="flex items-center gap-3">
          <ModeToggle compact />
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
      </div>

      {/* Row 2: Balance */}
      <div class="py-2 text-center border-b-2 border-border-grid" data-testid="header-balance">
        <Show when={!props.balanceLoading} fallback={
          <span class="text-xl font-mono font-bold text-text-dim">...</span>
        }>
          <Show when={props.balance !== null} fallback={
            <span class="text-xl font-mono font-bold text-text-dim">--</span>
          }>
            <span class="text-xl font-mono font-bold text-text-primary">
              {formatBalance(props.balance!)}
            </span>
            <span class="text-xs font-mono text-text-dim ml-2">USDT</span>
          </Show>
        </Show>
      </div>
    </div>
  );
}
