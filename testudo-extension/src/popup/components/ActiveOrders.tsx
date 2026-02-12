import { createSignal, onMount, onCleanup, For, Show } from "solid-js";
import browser from "webextension-polyfill";
import type { TradeGroupResponse } from "../../types";
import PositionCard from "./PositionCard";

interface ActiveOrdersProps {
  onCountChange?: (count: number) => void;
}

export default function ActiveOrders(props: ActiveOrdersProps) {
  const [trades, setTrades] = createSignal<TradeGroupResponse[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal("");

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
    await browser.runtime.sendMessage({ type: "CANCEL_TRADE", tradeId });
    fetchTrades();
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

  const activeTrades = () => {
    const active = trades().filter((t) => t.status !== "Completed");
    props.onCountChange?.(active.length);
    return active;
  };

  return (
    <div class="px-4 py-3" data-testid="active-orders">
      <div class="flex items-center justify-between mb-3">
        <label class="text-[11px] text-signal-orange uppercase tracking-[0.15em] font-bold">
          <Show when={!loading() && activeTrades().length > 0} fallback="Active Positions">
            <span class="text-text-primary">{activeTrades().length}</span> Active Positions
          </Show>
        </label>
        <button
          class="px-2 py-0.5 text-xs border-0 text-text-dim hover:text-text-secondary hover:bg-transparent"
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
        <p class="text-[13px] text-text-dim font-mono py-2">Loading...</p>
      </Show>

      <Show when={error() && !loading()}>
        <p class="text-[13px] text-signal-red font-mono py-2" data-testid="orders-error">{error()}</p>
      </Show>

      <Show when={!loading() && !error() && activeTrades().length === 0}>
        <div class="flex flex-col items-center justify-center py-8" data-testid="empty-positions">
          <p class="text-[13px] font-display text-text-dim tracking-[0.2em]">NO ACTIVE POSITIONS</p>
          <p class="text-[11px] text-text-dim font-mono mt-2">
            Trades placed via TradingView will
          </p>
          <p class="text-[11px] text-text-dim font-mono">
            appear here automatically.
          </p>
        </div>
      </Show>

      <div class="space-y-2">
        <For each={activeTrades()}>
          {(trade) => (
            <PositionCard
              trade={trade}
              onCancel={handleCancel}
            />
          )}
        </For>
      </div>
    </div>
  );
}
