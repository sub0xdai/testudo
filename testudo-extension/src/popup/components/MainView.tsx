import { createSignal, onMount, onCleanup, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";
import HeaderBar from "./HeaderBar";
import TabBar, { type TabId } from "./TabBar";
import TradeManagement from "./TradeManagement";
import ActiveOrders from "./ActiveOrders";
import StatusBar from "./StatusBar";
import type { BalanceResponse } from "../../types";

function formatBalance(value: number): string {
  return value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

export default function MainView(props: { onOpenSettings: () => void }) {
  const auth = useAuth();
  const [activeTab, setActiveTab] = createSignal<TabId>("trade");
  const [balance, setBalance] = createSignal<BalanceResponse[] | null>(null);
  const [balanceLoading, setBalanceLoading] = createSignal(true);
  const [positionCount, setPositionCount] = createSignal(0);

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
  const total = () => {
    const a = available();
    const l = locked();
    if (a === null && l === null) return null;
    return (a || 0) + (l || 0);
  };

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
      {/* Persistent Header with Balance */}
      <HeaderBar
        balance={total()}
        balanceLoading={balanceLoading()}
        onOpenSettings={props.onOpenSettings}
      />

      {/* Tab Navigation */}
      <TabBar
        activeTab={activeTab()}
        onTabChange={setActiveTab}
        positionCount={positionCount()}
      />

      {/* Tab Content */}
      <div class="flex-1 overflow-y-auto">
        <Show when={activeTab() === "trade"}>
          <TradeManagement />
        </Show>

        <Show when={activeTab() === "positions"}>
          <ActiveOrders onCountChange={setPositionCount} />
        </Show>

        <Show when={activeTab() === "account"}>
          <div class="px-5 py-4 space-y-5" data-testid="balance-section">
            <div>
              <label class="block text-[11px] text-text-secondary uppercase tracking-[0.15em] font-bold mb-3">
                Balance
              </label>
              <Show when={!balanceLoading()} fallback={
                <p class="text-[13px] text-text-dim font-mono">...</p>
              }>
                <Show when={available() !== null} fallback={
                  <p class="text-[13px] text-text-dim italic font-mono">unavailable</p>
                }>
                  <div class="border border-border-grid card-depth">
                    <div class="flex items-center justify-between px-4 py-3 border-b border-border-grid bg-bg-panel">
                      <span class="text-[11px] text-text-dim uppercase tracking-wider">Available</span>
                      <span class="text-sm text-signal-green font-mono font-bold" data-testid="balance-available">
                        {formatBalance(available()!)}
                      </span>
                    </div>
                    <div class="flex items-center justify-between px-4 py-3 bg-bg-panel">
                      <span class="text-[11px] text-text-dim uppercase tracking-wider">Locked</span>
                      <span class="text-sm text-signal-orange font-mono font-bold" data-testid="balance-locked">
                        {formatBalance(locked()!)}
                      </span>
                    </div>
                  </div>
                </Show>
              </Show>
            </div>

            <div class="divider" />

            <div>
              <label class="block text-[11px] text-text-secondary uppercase tracking-[0.15em] font-bold mb-2">
                Connection
              </label>
              <StatusBar />
            </div>

            <div class="divider" />

            <div>
              <label class="block text-[11px] text-text-secondary uppercase tracking-[0.15em] font-bold mb-2">
                Account
              </label>
              <Show when={auth.email()}>
                <p class="text-xs font-mono text-text-secondary">{auth.email()}</p>
              </Show>
              <Show when={auth.paperOnly()}>
                <p class="text-[11px] text-text-dim font-mono">Paper mode</p>
              </Show>
            </div>
          </div>
        </Show>
      </div>

      {/* Minimal Footer */}
      <div class="px-5 py-1.5 border-t border-border-subtle flex items-center justify-between">
        <Show when={auth.email()}>
          <span class="text-[10px] text-text-dim truncate max-w-[200px]" data-testid="footer-email">
            {auth.email()}
          </span>
        </Show>
        <Show when={auth.paperOnly()}>
          <span class="text-[10px] text-text-dim tracking-wider" data-testid="footer-paper">
            PAPER ONLY
          </span>
        </Show>
      </div>
    </div>
  );
}
