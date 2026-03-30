import { createSignal, onMount, onCleanup, Show } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";
import HeaderBar from "./HeaderBar";
import ArcGauge from "./ArcGauge";
import TabBar, { type TabId } from "./TabBar";
import TradeManagement from "./TradeManagement";
import ActiveOrders from "./ActiveOrders";
import { DESK_URL, type ExchangeMode } from "../../utils";
import type { BalanceResponse, LiveBalanceResponse } from "../../types";

function formatBalance(value: number): string {
  return value.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export default function MainView(props: { onLogout: () => void }) {
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
  const [pendingCount, setPendingCount] = createSignal(0);
  const [exchangeMode, setExchangeMode] = createSignal<ExchangeMode>("cex");
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

  const usdt = () => balance()?.find((b) => b.asset === "USDT" || b.asset === "USDC");
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
        || status === "closed"
        || status === "cancelled";
      if (shouldRefresh) {
        if (balanceRefreshTimer) clearTimeout(balanceRefreshTimer);
        balanceRefreshTimer = setTimeout(() => {
          balanceRefreshTimer = null;
          fetchBalance();
        }, 250);
      }
    }
  }

  // Re-fetch balance when active exchange or mode changes
  function handleStorageChange(changes: Record<string, { oldValue?: unknown; newValue?: unknown }>) {
    if (changes.exchangeMode) {
      setExchangeMode((changes.exchangeMode.newValue as ExchangeMode) || "cex");
    }
    if (changes.activeCexAccountId || changes.activeDexAccountId || changes.exchangeMode) {
      setBalance(null);        // Clear stale balance before fetching
      setBalanceLoading(true);
      fetchBalance();
    }
  }

  onMount(async () => {
    const stored = await browser.storage.local.get(["popupActiveTab"]);
    if (stored.popupActiveTab) setActiveTabRaw(stored.popupActiveTab as TabId);
    const modeRes = await browser.runtime.sendMessage({ type: "GET_EXCHANGE_MODE" }) as { mode: ExchangeMode };
    if (modeRes?.mode) setExchangeMode(modeRes.mode);
    fetchBalance();
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
      <header>
        <HeaderBar onLogout={props.onLogout} />
      </header>

      {/* Wallet-style balance panel */}
      <div class="balance-panel" data-testid="header-balance">
        <div class="balance-panel-overlay" aria-hidden="true" />
        <div class="flex flex-col items-center pt-5 pb-2">
          <div class="flex items-center gap-2 mb-2">
            <span class="text-[12px] font-medium text-text-primary/70 tracking-widest uppercase">
              Balance
            </span>
            <Show when={exchangeName()}>
              {/* UXP-17: white badge instead of accent-green */}
              <span class="text-[10px] font-bold text-text-primary bg-text-primary/10 px-1.5 py-0.5 tracking-wider uppercase">
                {exchangeName()}
              </span>
            </Show>
          </div>
          <span
            class="text-[42px] font-bold text-text-primary tracking-tight leading-none text-shadow-balance"
          >
            <Show
              when={!balanceLoading() && total() !== null}
              fallback={
                <Show
                  when={!balanceLoading() && noExchange()}
                  fallback={<span class="balance-loading text-text-primary/60">$--</span>}
                >
                  <div class="mx-4 my-3 p-4 border border-text-primary/10 text-center">
                    <p class="text-sm text-text-dim mb-2">
                      {exchangeMode() === "dex" ? "No wallet connected" : "No CEX exchange linked"}
                    </p>
                    {/* UXP-17: white outline instead of accent bg */}
                    <button
                      class="px-4 py-1.5 text-xs font-medium border border-text-primary text-text-primary hover:bg-text-primary hover:text-bg-core transition cursor-pointer"
                      onClick={() => window.open(`${DESK_URL}/account?source=extension`, "_blank")}
                      data-testid="connect-account-cta"
                    >
                      {exchangeMode() === "dex" ? "Connect Wallet" : "Connect Account"}
                    </button>
                  </div>
                </Show>
              }
            >
              ${formatBalance(total()!)}
            </Show>
          </span>
          {/* Loading hint */}
          <Show when={balanceLoading()}>
            <span class="text-[12px] text-text-dim font-sans mt-1">Fetching balance...</span>
          </Show>
          {/* Delta line: available / locked breakdown — KEEP: trading data */}
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
        pendingCount={pendingCount()}
      />

      {/* Content — "control deck" with denser background */}
      <main class="flex-1 min-h-0 scroll-area bg-bg-panel">
        <Show when={activeTab() === "trade"}>
          <div role="tabpanel" id="panel-trade" aria-labelledby="tab-trade">
            <TradeManagement />
          </div>
        </Show>

        <Show when={activeTab() === "positions"}>
          <div role="tabpanel" id="panel-positions" aria-labelledby="tab-positions">
          <ActiveOrders
            onCountChange={(active, pending) => {
              setPositionCount(active);
              setPendingCount(pending);
            }}
            onBalanceRefresh={fetchBalance}
            isDex={exchangeMode() === "dex"}
          />
          </div>
        </Show>

        <Show when={activeTab() === "account"}>
          <div role="tabpanel" id="panel-account" aria-labelledby="tab-account" class="px-5 py-4 space-y-4" data-testid="balance-section">
            {/* Info Grid */}
            <Show
              when={!balanceLoading()}
              fallback={
                <p class="text-[14px] text-text-dim font-sans">Loading...</p>
              }
            >
              <Show
                when={available() !== null}
                fallback={
                  <p class="text-[14px] text-text-dim italic font-sans">
                    {noExchange()
                      ? (exchangeMode() === "dex" ? "No wallet connected" : "No CEX exchange linked")
                      : "Balance unavailable"}
                  </p>
                }
              >
                <div class="info-grid">
                  <div class="info-grid-cell">
                    <span class="block text-[11px] text-text-secondary font-sans font-medium mb-1">
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
                    <span class="block text-[11px] text-text-secondary font-sans font-medium mb-1">
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
                    <span class="block text-[11px] text-text-secondary font-sans font-medium mb-1">
                      Positions
                    </span>
                    <span class="text-[15px] text-text-primary font-mono font-bold">
                      {positionCount()}
                    </span>
                  </div>
                  <div class="info-grid-cell">
                    <span class="block text-[11px] text-text-secondary font-sans font-medium mb-1">
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

            {/* Account */}
            <div>
              <span class="block text-[12px] text-text-secondary font-sans font-medium mb-2">
                Account
              </span>
              <Show when={auth.walletAddress()}>
                <p class="text-[13px] font-mono text-text-secondary">
                  {auth.walletAddress()}
                </p>
              </Show>
            </div>

            <div class="divider" />

            {/* Support */}
            <div>
              <span class="block text-[12px] text-text-secondary font-sans font-medium mb-2">
                Support
              </span>
              <a
                href="mailto:support@testudo.vip"
                class="text-[12px] font-mono text-text-dim hover:text-text-secondary transition-colors"
              >
                support@testudo.vip
              </a>
            </div>
          </div>
        </Show>
      </main>

      {/* Footer */}
      <div class="px-5 py-2 border-t border-border-subtle flex items-center justify-between">
        <Show when={auth.walletAddress()}>
          <span
            class="text-[11px] text-text-dim font-sans truncate max-w-[200px]"
            data-testid="footer-wallet"
          >
            {auth.walletAddress()}
          </span>
        </Show>
      </div>
    </div>
  );
}
