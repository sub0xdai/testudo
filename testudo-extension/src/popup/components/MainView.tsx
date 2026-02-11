import { createSignal, onMount, onCleanup, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";
import TradeManagement from "./TradeManagement";
import ActiveOrders from "./ActiveOrders";
import ModeToggle from "./ModeToggle";
import StatusBar from "./StatusBar";
import type { BalanceResponse } from "../../types";

function formatBalance(value: number): string {
  return value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

export default function MainView(props: { onOpenSettings: () => void }) {
  const auth = useAuth();
  const [balance, setBalance] = createSignal<BalanceResponse[] | null>(null);
  const [balanceLoading, setBalanceLoading] = createSignal(true);

  async function fetchBalance() {
    try {
      const resp = await browser.runtime.sendMessage({ type: "GET_BALANCES" }) as {
        success?: boolean;
        data?: BalanceResponse[];
      };
      if (resp?.success && resp.data) setBalance(resp.data);
    } catch { /* non-blocking */ }
    setBalanceLoading(false);
  }

  const usdt = () => balance()?.find((b) => b.asset === "USDT");
  const available = () => usdt() ? parseFloat(usdt()!.available) : null;
  const locked = () => usdt() ? parseFloat(usdt()!.locked) : null;

  function handleMessage(message: unknown) {
    const msg = message as { type: string };
    if (msg.type === "WS_ORDER_UPDATE") {
      fetchBalance();
    }
  }

  onMount(() => {
    fetchBalance();
    browser.runtime.onMessage.addListener(handleMessage);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

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
        <div class="border-t-2 border-border-grid pt-3" data-testid="balance-section">
          <label class="block text-[13px] text-signal-orange uppercase tracking-widest font-bold mb-2">
            Account
          </label>
          <Show when={!balanceLoading()} fallback={
            <p class="text-[13px] text-text-dim font-mono">...</p>
          }>
            <Show when={available() !== null} fallback={
              <p class="text-[13px] text-text-dim italic font-mono">unavailable</p>
            }>
              <div class="flex items-center justify-between">
                <span class="text-[13px] text-text-secondary">Available</span>
                <span class="text-sm text-signal-green font-mono" data-testid="balance-available">
                  {formatBalance(available()!)} USDT
                </span>
              </div>
              <div class="flex items-center justify-between mt-1">
                <span class="text-[13px] text-text-secondary">Locked</span>
                <span class="text-sm text-signal-orange font-mono" data-testid="balance-locked">
                  {formatBalance(locked()!)} USDT
                </span>
              </div>
            </Show>
          </Show>
        </div>
        <ModeToggle />
      </div>

      {/* Footer */}
      <div class="px-4 py-2 border-t-2 border-border-grid flex items-center justify-between">
        <StatusBar />
        <Show when={auth.email()}>
          <span class="text-[13px] text-text-secondary truncate max-w-[140px]" data-testid="footer-email">
            {auth.email()}
          </span>
        </Show>
        <Show when={auth.paperOnly()}>
          <span class="text-[13px] text-text-secondary" data-testid="footer-paper">
            PAPER ONLY
          </span>
        </Show>
      </div>
    </div>
  );
}
