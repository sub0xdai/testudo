import { createSignal, onMount, onCleanup, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";
import HeaderBar from "./HeaderBar";
import ArcGauge from "./ArcGauge";
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
  const exposure = () => {
    const t = total();
    const l = locked();
    if (!t || t === 0) return 0;
    return ((l || 0) / t) * 100;
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
    <div class="flex flex-col h-full">
      {/* Compact Header */}
      <HeaderBar onOpenSettings={props.onOpenSettings} />

      {/* Arc Gauge — hero visual */}
      <div class="border-b border-border-subtle" data-testid="header-balance">
        <ArcGauge
          exposure={exposure()}
          atRisk={locked() || 0}
          totalBalance={total() || 0}
        />
      </div>

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
          <div class="px-5 py-4 space-y-4" data-testid="balance-section">
            {/* Info Grid */}
            <Show when={!balanceLoading()} fallback={
              <p class="text-[13px] text-text-dim font-mono">Loading...</p>
            }>
              <Show when={available() !== null} fallback={
                <p class="text-[13px] text-text-dim italic font-sans">Balance unavailable</p>
              }>
                <div class="info-grid">
                  <div class="info-grid-cell">
                    <span class="block text-[10px] text-text-dim font-sans uppercase tracking-wider mb-1">Available</span>
                    <span class="text-[15px] text-signal-green font-mono font-bold" data-testid="balance-available">
                      {formatBalance(available()!)}
                    </span>
                  </div>
                  <div class="info-grid-cell">
                    <span class="block text-[10px] text-text-dim font-sans uppercase tracking-wider mb-1">Locked</span>
                    <span class="text-[15px] text-signal-orange font-mono font-bold" data-testid="balance-locked">
                      {formatBalance(locked()!)}
                    </span>
                  </div>
                  <div class="info-grid-cell">
                    <span class="block text-[10px] text-text-dim font-sans uppercase tracking-wider mb-1">Positions</span>
                    <span class="text-[15px] text-text-primary font-mono font-bold">
                      {positionCount()}
                    </span>
                  </div>
                  <div class="info-grid-cell">
                    <span class="block text-[10px] text-text-dim font-sans uppercase tracking-wider mb-1">Exposure</span>
                    <span class={`text-[15px] font-mono font-bold ${
                      exposure() > 50 ? "text-signal-red" : exposure() > 25 ? "text-signal-orange" : "text-signal-green"
                    }`}>
                      {exposure().toFixed(1)}%
                    </span>
                  </div>
                </div>
              </Show>
            </Show>

            <div class="divider" />

            {/* Connection */}
            <div>
              <span class="block text-[10px] text-text-dim font-sans uppercase tracking-wider mb-2">Connection</span>
              <StatusBar />
            </div>

            <div class="divider" />

            {/* Account */}
            <div>
              <span class="block text-[10px] text-text-dim font-sans uppercase tracking-wider mb-2">Account</span>
              <Show when={auth.email()}>
                <p class="text-[13px] font-mono text-text-secondary">{auth.email()}</p>
              </Show>
              <Show when={auth.paperOnly()}>
                <p class="text-[11px] text-text-dim font-sans">Paper mode</p>
              </Show>
            </div>
          </div>
        </Show>
      </div>

      {/* Minimal Footer */}
      <div class="px-5 py-1.5 border-t border-border-subtle flex items-center justify-between">
        <Show when={auth.email()}>
          <span class="text-[10px] text-text-dim font-sans truncate max-w-[200px]" data-testid="footer-email">
            {auth.email()}
          </span>
        </Show>
        <Show when={auth.paperOnly()}>
          <span class="text-[10px] text-text-dim font-sans tracking-wider" data-testid="footer-paper">
            PAPER ONLY
          </span>
        </Show>
      </div>
    </div>
  );
}
