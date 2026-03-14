import { createSignal, createMemo, createEffect, onMount, onCleanup, For, Show } from "solid-js";
import browser from "webextension-polyfill";
import type { TradeGroupResponse, ExchangePositionsResponse } from "../../types";
import PositionCard from "./PositionCard";

interface ActiveOrdersProps {
  onCountChange?: (activeCount: number, pendingCount: number) => void;
  onBalanceRefresh?: () => void;
}

export default function ActiveOrders(props: ActiveOrdersProps) {
  const [trades, setTrades] = createSignal<TradeGroupResponse[]>([]);
  const [exchangeData, setExchangeData] = createSignal<ExchangePositionsResponse | null>(null);
  const [exchangeLoading, setExchangeLoading] = createSignal(false);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal("");
  const [cancelError, setCancelError] = createSignal("");
  const [cancelling, setCancelling] = createSignal("");

  async function fetchExchangePositions() {
    setExchangeLoading(true);
    try {
      const response = await browser.runtime.sendMessage({ type: "EXCHANGE_POSITIONS" }) as {
        success: boolean;
        data?: ExchangePositionsResponse;
        error?: string;
      };
      if (response.success && response.data) {
        setExchangeData(response.data);
      }
    } catch {
      // Silent — fallback is best-effort
    }
    setExchangeLoading(false);
  }

  async function fetchTrades() {
    try {
      const response = await browser.runtime.sendMessage({ type: "LIST_TRADES" }) as {
        success: boolean;
        data?: TradeGroupResponse[];
        error?: string;
      };

      if (response.success && response.data) {
        setTrades(response.data);
        setError("");
        // If no tracked trades, try exchange fallback
        if (response.data.length === 0) {
          fetchExchangePositions();
        } else {
          setExchangeData(null);
        }
      } else {
        setError(response.error || "Failed to load");
      }
    } catch {
      setError("Connection failed");
    }
    setLoading(false);
  }

  async function handleCancel(tradeId: string) {
    setCancelError("");
    setCancelling(tradeId);
    try {
      const response = await browser.runtime.sendMessage({ type: "CANCEL_TRADE", tradeId }) as {
        success: boolean;
        error?: string;
      };
      if (response.success) {
        fetchTrades();
        props.onBalanceRefresh?.();
      } else {
        setCancelError(response.error || "Cancel failed");
      }
    } catch {
      setCancelError("Failed to send cancel request");
    } finally {
      setCancelling("");
    }
  }

  let fetchTradesTimer: ReturnType<typeof setTimeout> | null = null;

  function handleMessage(message: unknown) {
    const msg = message as { type: string };
    if (msg.type === "WS_ORDER_UPDATE") {
      if (fetchTradesTimer) clearTimeout(fetchTradesTimer);
      fetchTradesTimer = setTimeout(() => {
        fetchTradesTimer = null;
        fetchTrades();
      }, 250);
    }
  }

  onMount(() => {
    fetchTrades();
    browser.runtime.onMessage.addListener(handleMessage);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
    if (fetchTradesTimer) {
      clearTimeout(fetchTradesTimer);
      fetchTradesTimer = null;
    }
  });

  const positions = createMemo(() => trades().filter((t) => t.status === "Active"));
  const pendingOrders = createMemo(() => trades().filter((t) => t.status === "Pending"));

  createEffect(() => {
    props.onCountChange?.(positions().length, pendingOrders().length);
  });

  return (
    <div class="px-5 py-4" data-testid="active-orders">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-[13px] text-text-primary font-sans font-medium m-0">
          Positions
        </h2>
        <button
          class="icon-btn border-0 rounded-lg text-text-dim hover:text-text-secondary hover:bg-bg-elevated"
          onClick={fetchTrades}
          title="Refresh"
          data-testid="refresh-orders"
        >
          <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
            <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
            <path d="M21 21v-5h-5" />
          </svg>
        </button>
      </div>

      <Show when={loading()}>
        <p class="text-[13px] text-text-dim font-sans py-2">Loading...</p>
      </Show>

      <Show when={error() && !loading()}>
        <p role="alert" class="text-[13px] text-signal-red font-sans py-2" data-testid="orders-error">{error()}</p>
      </Show>

      <Show when={cancelError()}>
        <p role="alert" class="text-[13px] text-signal-red font-sans py-2" data-testid="cancel-error">{cancelError()}</p>
      </Show>

      <Show when={!loading() && !error() && positions().length === 0 && pendingOrders().length === 0}>
        <Show when={exchangeLoading()}>
          <p class="text-[13px] text-text-dim font-sans py-2">Checking exchange...</p>
        </Show>

        {/* Exchange positions fallback */}
        <Show when={!exchangeLoading() && exchangeData() && (exchangeData()!.positions.length > 0 || exchangeData()!.open_orders.length > 0)}>
          <div class="mb-3">
            <div class="flex items-center gap-2 mb-3">
              <span class="w-2 h-2 rounded-full bg-signal-blue" />
              <span class="text-[12px] text-text-secondary font-sans font-medium uppercase tracking-wider">
                From Exchange
              </span>
              <span class="text-[10px] font-mono text-text-dim bg-bg-elevated px-1.5 py-0.5 rounded">
                {exchangeData()!.exchange_name}
              </span>
            </div>
            <p class="text-[11px] text-text-dim font-sans mb-3">
              These positions exist on the exchange but aren't tracked by Testudo.
            </p>
          </div>

          <Show when={exchangeData()!.positions.length > 0}>
            <div class="mb-4">
              <div class="flex items-center gap-2 mb-2">
                <span class="text-[11px] text-text-dim font-sans font-medium uppercase tracking-wider">
                  Positions
                </span>
                <span class="text-[11px] font-mono text-signal-blue bg-signal-blue/10 px-1.5 py-0.5 rounded-full">
                  {exchangeData()!.positions.length}
                </span>
              </div>
              <div class="space-y-2">
                <For each={exchangeData()!.positions}>
                  {(pos) => (
                    <div class="bg-bg-panel border border-border-subtle rounded-xl p-3">
                      <div class="flex items-center justify-between mb-1">
                        <span class="text-[13px] font-mono font-medium text-text-primary">{pos.symbol}</span>
                        <span class={`text-[11px] font-mono font-semibold px-2 py-0.5 rounded ${pos.side.toLowerCase() === "long" ? "text-signal-green bg-signal-green/10" : "text-signal-red bg-signal-red/10"}`}>
                          {pos.side}
                        </span>
                      </div>
                      <div class="grid grid-cols-3 gap-2 text-[11px] font-mono">
                        <div>
                          <span class="text-text-dim">Size</span>
                          <p class="text-text-secondary">{pos.contracts}</p>
                        </div>
                        <div>
                          <span class="text-text-dim">Entry</span>
                          <p class="text-text-secondary">{pos.entry_price}</p>
                        </div>
                        <div>
                          <span class="text-text-dim">uPnL</span>
                          <p class={parseFloat(pos.unrealized_pnl) >= 0 ? "text-signal-green" : "text-signal-red"}>
                            {parseFloat(pos.unrealized_pnl) >= 0 ? "+" : ""}{parseFloat(pos.unrealized_pnl).toFixed(2)}
                          </p>
                        </div>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>

          <Show when={exchangeData()!.open_orders.length > 0}>
            <div>
              <div class="flex items-center gap-2 mb-2">
                <span class="text-[11px] text-text-dim font-sans font-medium uppercase tracking-wider">
                  Open Orders
                </span>
                <span class="text-[11px] font-mono text-signal-orange bg-signal-orange/10 px-1.5 py-0.5 rounded-full">
                  {exchangeData()!.open_orders.length}
                </span>
              </div>
              <div class="space-y-2">
                <For each={exchangeData()!.open_orders}>
                  {(order) => (
                    <div class="bg-bg-panel border border-border-subtle rounded-xl p-3">
                      <div class="flex items-center justify-between mb-1">
                        <span class="text-[12px] font-mono text-text-primary">{order.symbol}</span>
                        <div class="flex items-center gap-1.5">
                          <span class={`text-[10px] font-mono px-1.5 py-0.5 rounded ${order.side.toLowerCase() === "buy" ? "text-signal-green bg-signal-green/10" : "text-signal-red bg-signal-red/10"}`}>
                            {order.side}
                          </span>
                          <span class="text-[10px] font-mono text-text-dim bg-bg-elevated px-1.5 py-0.5 rounded">
                            {order.type}
                          </span>
                        </div>
                      </div>
                      <div class="flex gap-4 text-[11px] font-mono">
                        <div>
                          <span class="text-text-dim">Amt </span>
                          <span class="text-text-secondary">{order.amount}</span>
                        </div>
                        <Show when={order.price}>
                          <div>
                            <span class="text-text-dim">Price </span>
                            <span class="text-text-secondary">{order.price}</span>
                          </div>
                        </Show>
                        <Show when={order.stop_price}>
                          <div>
                            <span class="text-text-dim">Stop </span>
                            <span class="text-text-secondary">{order.stop_price}</span>
                          </div>
                        </Show>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>
        </Show>

        {/* True empty state — no tracked trades AND no exchange positions */}
        <Show when={!exchangeLoading() && (!exchangeData() || (exchangeData()!.positions.length === 0 && exchangeData()!.open_orders.length === 0))}>
          <div class="flex flex-col items-center justify-center py-12" data-testid="empty-positions">
            <div class="w-12 h-12 rounded-2xl bg-bg-panel border border-border-subtle flex items-center justify-center mb-4">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="text-text-dim">
                <path d="M3 3h18v18H3z" />
                <path d="M3 9h18M9 21V9" />
              </svg>
            </div>
            <p class="text-[14px] font-sans font-medium text-text-secondary">No active positions</p>
            <p class="text-[12px] text-text-dim font-sans mt-2 text-center leading-relaxed">
              Trades placed via TradingView will<br />appear here automatically.
            </p>
          </div>
        </Show>
      </Show>

      {/* Active Positions */}
      <Show when={!loading() && !error() && positions().length > 0}>
        <div class="mb-4">
          <div class="flex items-center gap-2 mb-3">
            <span class="w-2 h-2 rounded-full bg-signal-green" />
            <span class="text-[12px] text-text-secondary font-sans font-medium uppercase tracking-wider">
              Active
            </span>
            <span class="text-[11px] font-mono text-signal-green bg-signal-green/10 px-1.5 py-0.5 rounded-full">
              {positions().length}
            </span>
          </div>
          <div class="space-y-3">
            <For each={positions()}>
              {(trade) => (
                <PositionCard
                  trade={trade}
                  onCancel={handleCancel}
                  cancelling={cancelling() === trade.id}
                />
              )}
            </For>
          </div>
        </div>
      </Show>

      {/* Pending Orders */}
      <Show when={!loading() && !error() && pendingOrders().length > 0}>
        <div>
          <div class="flex items-center gap-2 mb-3">
            <span class="w-2 h-2 rounded-full bg-signal-orange animate-pulse" />
            <span class="text-[12px] text-text-secondary font-sans font-medium uppercase tracking-wider">
              Pending
            </span>
            <span class="text-[11px] font-mono text-signal-orange bg-signal-orange/10 px-1.5 py-0.5 rounded-full">
              {pendingOrders().length}
            </span>
          </div>
          <div class="space-y-3">
            <For each={pendingOrders()}>
              {(trade) => (
                <PositionCard
                  trade={trade}
                  onCancel={handleCancel}
                  cancelling={cancelling() === trade.id}
                />
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
}
