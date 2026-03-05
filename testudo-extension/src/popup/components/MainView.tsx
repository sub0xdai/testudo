import { createSignal, onMount, onCleanup, Show, For } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";
import HeaderBar from "./HeaderBar";
import ArcGauge from "./ArcGauge";
import TabBar, { type TabId } from "./TabBar";
import TradeManagement from "./TradeManagement";
import QuickTrade from "./QuickTrade";
import ActiveOrders from "./ActiveOrders";
import StatusBar from "./StatusBar";
import type { BalanceResponse, LiveBalanceResponse, ScraperHealthRecord } from "../../types";

function formatBalance(value: number): string {
  return value.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export default function MainView(props: { onOpenSettings: () => void }) {
  const auth = useAuth();
  const [activeTab, setActiveTabRaw] = createSignal<TabId>("trade");

  function setActiveTab(tab: TabId) {
    setActiveTabRaw(tab);
    browser.storage.local.set({ popupActiveTab: tab });
  }
  const [balance, setBalance] = createSignal<BalanceResponse[] | null>(null);
  const [exchangeName, setExchangeName] = createSignal<string | undefined>();
  const [noExchange, setNoExchange] = createSignal(false);
  const [balanceLoading, setBalanceLoading] = createSignal(true);
  const [positionCount, setPositionCount] = createSignal(0);
  const [scraperHealth, setScraperHealth] = createSignal<ScraperHealthRecord[]>(
    [],
  );
  let balanceRefreshTimer: ReturnType<typeof setTimeout> | null = null;

  async function fetchBalance() {
    try {
      const resp = (await browser.runtime.sendMessage({
        type: "GET_BALANCE",
      })) as {
        success?: boolean;
        data?: LiveBalanceResponse;
        error?: string;
      };
      if (resp?.success && resp.data) {
        setBalance(resp.data.balances);
        setExchangeName(resp.data.exchange_name);
        setNoExchange(false);
      } else if (resp?.error === "No active exchange selected") {
        setNoExchange(true);
      }
    } catch {
      /* non-blocking */
    }
    setBalanceLoading(false);
  }

  const usdt = () => balance()?.find((b) => b.asset === "USDT");
  const available = () => (usdt() ? parseFloat(usdt()!.available) : null);
  const locked = () => (usdt() ? parseFloat(usdt()!.locked) : null);
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
    const msg = message as { type: string; data?: { status?: string; e?: string } };
    if (msg.type === "WS_ORDER_UPDATE") {
      const status = msg.data?.status || msg.data?.e;
      const shouldRefresh = !status
        || status === "stopped_out"
        || status === "took_profit"
        || status === "entry_filled"
        || status === "closed";
      if (shouldRefresh) {
        if (balanceRefreshTimer) clearTimeout(balanceRefreshTimer);
        balanceRefreshTimer = setTimeout(() => {
          balanceRefreshTimer = null;
          fetchBalance();
        }, 250);
      }
    }
  }

  // Re-fetch balance when active exchange changes
  function handleStorageChange(changes: Record<string, { oldValue?: unknown; newValue?: unknown }>) {
    if (changes.activeExchangeId) {
      setBalanceLoading(true);
      fetchBalance();
    }
  }

  async function fetchScraperHealth() {
    try {
      const stored = await browser.storage.local.get(["scraperHealth"]);
      if (stored.scraperHealth)
        setScraperHealth(stored.scraperHealth as ScraperHealthRecord[]);
    } catch {
      /* non-blocking */
    }
  }

  const scraperSuccessRate = () => {
    const records = scraperHealth();
    if (records.length === 0) return null;
    const successes = records.filter((r) => r.success).length;
    return Math.round((successes / records.length) * 100);
  };

  const lastStrategy = () => {
    const records = scraperHealth();
    if (records.length === 0) return null;
    return records[records.length - 1];
  };

  onMount(async () => {
    const stored = await browser.storage.local.get(["popupActiveTab"]);
    if (stored.popupActiveTab) setActiveTabRaw(stored.popupActiveTab as TabId);
    fetchBalance();
    fetchScraperHealth();
    browser.runtime.onMessage.addListener(handleMessage);
    browser.storage.onChanged.addListener(handleStorageChange);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
    browser.storage.onChanged.removeListener(handleStorageChange);
    if (balanceRefreshTimer) {
      clearTimeout(balanceRefreshTimer);
      balanceRefreshTimer = null;
    }
  });

  return (
    <div class="flex flex-col h-full">
      {/* Slim toolbar */}
      <HeaderBar onOpenSettings={props.onOpenSettings} />

      {/* Wallet-style balance panel */}
      <div class="balance-panel" data-testid="header-balance">
        <div class="balance-panel-overlay" aria-hidden="true" />
        <div class="flex flex-col items-center pt-5 pb-2">
          <div class="flex items-center gap-2 mb-2">
            <span class="text-[12px] font-medium text-white/70 tracking-widest uppercase">
              Balance
            </span>
            <Show when={exchangeName()}>
              <span class="text-[10px] font-bold text-accent-green bg-accent-green/15 px-1.5 py-0.5 rounded tracking-wider uppercase">
                {exchangeName()}
              </span>
            </Show>
          </div>
          <span
            class="text-[42px] font-bold text-white tracking-tight leading-none"
            style={{ "text-shadow": "0 1px 8px rgba(0,0,0,0.6)" }}
          >
            <Show
              when={!balanceLoading() && total() !== null}
              fallback={
                <Show
                  when={!balanceLoading() && noExchange()}
                  fallback={<span class="text-white/60">$--</span>}
                >
                  <span class="text-[18px] text-text-dim font-sans font-medium">
                    Connect an exchange in Settings
                  </span>
                </Show>
              }
            >
              ${formatBalance(total()!)}
            </Show>
          </span>
          {/* Delta line: available / locked breakdown */}
          <Show when={!balanceLoading() && available() !== null}>
            <div class="flex items-center gap-2 mt-2">
              <span class="text-[12px] text-signal-green font-medium">
                ${formatBalance(available()!)} available
              </span>
              <span class="text-text-dim">·</span>
              <span class="text-[12px] text-text-secondary">
                ${formatBalance(locked()!)} locked
              </span>
            </div>
          </Show>
        </div>

        {/* Risk gauge */}
        <ArcGauge
          exposure={exposure()}
          atRisk={locked() || 0}
          totalBalance={total() || 0}
        />
      </div>

      {/* Tabs */}
      <TabBar
        activeTab={activeTab()}
        onTabChange={setActiveTab}
        positionCount={positionCount()}
      />

      {/* Content */}
      <div class="flex-1 scroll-area">
        <Show when={activeTab() === "trade"}>
          <TradeManagement />
        </Show>

        <Show when={activeTab() === "quick"}>
          <QuickTrade />
        </Show>

        <Show when={activeTab() === "positions"}>
          <ActiveOrders
            onCountChange={setPositionCount}
            onBalanceRefresh={fetchBalance}
          />
        </Show>

        <Show when={activeTab() === "account"}>
          <div class="px-5 py-4 space-y-4" data-testid="balance-section">
            {/* Info Grid */}
            <Show
              when={!balanceLoading()}
              fallback={
                <p class="text-[14px] text-zinc-500 font-sans">Loading...</p>
              }
            >
              <Show
                when={available() !== null}
                fallback={
                  <p class="text-[14px] text-zinc-500 italic font-sans">
                    {noExchange() ? "Connect an exchange to view balance" : "Balance unavailable"}
                  </p>
                }
              >
                <div class="info-grid">
                  <div class="info-grid-cell">
                    <span class="block text-[11px] text-zinc-400 font-sans font-medium mb-1">
                      Available
                    </span>
                    <span
                      class="text-[15px] text-signal-green font-mono font-bold"
                      data-testid="balance-available"
                    >
                      {formatBalance(available()!)}
                    </span>
                  </div>
                  <div class="info-grid-cell">
                    <span class="block text-[11px] text-zinc-400 font-sans font-medium mb-1">
                      Locked
                    </span>
                    <span
                      class="text-[15px] text-signal-orange font-mono font-bold"
                      data-testid="balance-locked"
                    >
                      {formatBalance(locked()!)}
                    </span>
                  </div>
                  <div class="info-grid-cell">
                    <span class="block text-[11px] text-zinc-400 font-sans font-medium mb-1">
                      Positions
                    </span>
                    <span class="text-[15px] text-white font-mono font-bold">
                      {positionCount()}
                    </span>
                  </div>
                  <div class="info-grid-cell">
                    <span class="block text-[11px] text-zinc-400 font-sans font-medium mb-1">
                      Exposure
                    </span>
                    <span
                      class={`text-[15px] font-mono font-bold ${
                        exposure() > 50
                          ? "text-signal-red"
                          : exposure() > 25
                            ? "text-signal-orange"
                            : "text-signal-green"
                      }`}
                    >
                      {exposure().toFixed(1)}%
                    </span>
                  </div>
                </div>
              </Show>
            </Show>

            <div class="divider" />

            {/* Connection */}
            <div>
              <span class="block text-[12px] text-text-secondary font-sans font-medium mb-2">
                Connection
              </span>
              <StatusBar />
            </div>

            <div class="divider" />

            {/* Scraper Health */}
            <Show when={scraperHealth().length > 0}>
              <div data-testid="scraper-health">
                <span class="block text-[12px] text-text-secondary font-sans font-medium mb-2">
                  Auto-Fill Health
                </span>
                <div class="info-grid">
                  <div class="info-grid-cell">
                    <span class="block text-[11px] text-zinc-400 font-sans font-medium mb-1">
                      Success Rate
                    </span>
                    <span
                      class={`text-[15px] font-mono font-bold ${
                        scraperSuccessRate()! >= 80
                          ? "text-signal-green"
                          : scraperSuccessRate()! >= 50
                            ? "text-signal-orange"
                            : "text-signal-red"
                      }`}
                    >
                      {scraperSuccessRate()}%
                    </span>
                  </div>
                  <div class="info-grid-cell">
                    <span class="block text-[11px] text-zinc-400 font-sans font-medium mb-1">
                      Last Strategy
                    </span>
                    <span class="text-[15px] font-mono font-bold text-white">
                      {lastStrategy()?.success
                        ? `S${lastStrategy()!.strategyUsed}`
                        : "failed"}
                    </span>
                  </div>
                </div>
                <div
                  class="flex gap-0.5 mt-2"
                  title="Recent scraper results (green=success, red=fail)"
                >
                  <For each={scraperHealth().slice(-10)}>
                    {(record) => (
                      <div
                        class={`h-1.5 flex-1 rounded-full ${record.success ? "bg-signal-green" : "bg-signal-red"}`}
                        title={`Strategy ${record.strategyUsed ?? "none"} — ${record.success ? "OK" : "FAIL"}`}
                      />
                    )}
                  </For>
                </div>
              </div>
              <div class="divider" />
            </Show>

            {/* Account */}
            <div>
              <span class="block text-[12px] text-text-secondary font-sans font-medium mb-2">
                Account
              </span>
              <Show when={auth.email()}>
                <p class="text-[13px] font-mono text-text-secondary">
                  {auth.email()}
                </p>
              </Show>
            </div>
          </div>
        </Show>
      </div>

      {/* Footer */}
      <div class="px-5 py-2 border-t border-border-subtle flex items-center justify-between">
        <Show when={auth.email()}>
          <span
            class="text-[11px] text-zinc-500 font-sans truncate max-w-[200px]"
            data-testid="footer-email"
          >
            {auth.email()}
          </span>
        </Show>
      </div>
    </div>
  );
}
