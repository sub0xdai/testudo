import { createSignal, createEffect, onMount, onCleanup, For, Show } from "solid-js";
import browser from "webextension-polyfill";
import type { TradeGroupResponse } from "../../types";
import PositionCard from "./PositionCard";

interface ActiveOrdersProps {
  onCountChange?: (count: number) => void;
  onBalanceRefresh?: () => void;
}

export default function ActiveOrders(props: ActiveOrdersProps) {
  const [trades, setTrades] = createSignal<TradeGroupResponse[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal("");
  const [cancelError, setCancelError] = createSignal("");
  const [cancelling, setCancelling] = createSignal("");

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

  function handleMessage(message: unknown) {
    const msg = message as { type: string };
    if (msg.type === "WS_ORDER_UPDATE") {
      fetchTrades();
    }
  }

  onMount(() => {
    fetchTrades();
    browser.runtime.onMessage.addListener(handleMessage);
  });

  onCleanup(() => {
    browser.runtime.onMessage.removeListener(handleMessage);
  });

  const activeTrades = () => trades().filter((t) => t.status === "Active" || t.status === "Pending");

  createEffect(() => {
    props.onCountChange?.(activeTrades().length);
  });

  return (
    <div class="px-5 py-4" data-testid="active-orders">
      <div class="flex items-center justify-between mb-4">
        <span class="text-[13px] text-zinc-300 font-sans font-medium">
          <Show when={!loading() && activeTrades().length > 0} fallback="Active Positions">
            <span class="text-white font-mono font-bold">{activeTrades().length}</span>
            {" "}Active Positions
          </Show>
        </span>
        <button
          class="p-1.5 border-0 rounded-lg text-text-dim hover:text-text-secondary hover:bg-bg-elevated"
          onClick={fetchTrades}
          title="Refresh"
          data-testid="refresh-orders"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
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
        <p class="text-[13px] text-signal-red font-sans py-2" data-testid="orders-error">{error()}</p>
      </Show>

      <Show when={cancelError()}>
        <p class="text-[13px] text-signal-red font-sans py-2" data-testid="cancel-error">{cancelError()}</p>
      </Show>

      <Show when={!loading() && !error() && activeTrades().length === 0}>
        <div class="flex flex-col items-center justify-center py-12" data-testid="empty-positions">
          <div class="w-12 h-12 rounded-2xl bg-bg-panel border border-border-subtle flex items-center justify-center mb-4">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="text-text-dim">
              <path d="M3 3h18v18H3z" />
              <path d="M3 9h18M9 21V9" />
            </svg>
          </div>
          <p class="text-[14px] font-sans font-medium text-text-secondary" data-testid="empty-positions">No active positions</p>
          <p class="text-[12px] text-zinc-500 font-sans mt-2 text-center leading-relaxed">
            Trades placed via TradingView will<br />appear here automatically.
          </p>
        </div>
      </Show>

      <div class="space-y-3">
        <For each={activeTrades()}>
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
  );
}
