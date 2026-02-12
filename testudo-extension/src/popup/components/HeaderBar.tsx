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
    <div data-testid="header-bar" class="panel-depth">
      {/* Row 1: Logo + Status + Mode + Gear */}
      <div class="flex items-center justify-between px-5 pt-4 pb-2">
        <div class="flex items-center gap-3">
          <StatusBar />
          <h1 class="text-sm font-display font-bold tracking-[0.25em] text-text-primary">
            TESTUDO
          </h1>
        </div>
        <div class="flex items-center gap-3">
          <ModeToggle compact />
          <button
            class="p-1.5 border-0 text-text-dim hover:text-text-primary hover:bg-transparent"
            onClick={props.onOpenSettings}
            data-testid="settings-btn"
            title="Settings"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          </button>
        </div>
      </div>

      {/* Row 2: Hero Balance */}
      <div class="px-5 pb-4 pt-1" data-testid="header-balance">
        <Show when={!props.balanceLoading} fallback={
          <span class="text-[28px] font-mono font-bold text-text-dim">...</span>
        }>
          <Show when={props.balance !== null} fallback={
            <span class="text-[28px] font-mono font-bold text-text-dim">--</span>
          }>
            <span class="text-[28px] font-mono font-bold text-text-primary tracking-tight">
              {formatBalance(props.balance!)}
            </span>
            <span class="text-[11px] font-mono text-text-dim ml-2 tracking-wider">USDT</span>
          </Show>
        </Show>
      </div>
    </div>
  );
}
