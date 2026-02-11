import { createSignal, onMount, onCleanup, For, Show } from "solid-js";
import browser from "webextension-polyfill";
import type { TradeGroupResponse } from "../../types";

export default function ActiveOrders() {
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

  const activeTrades = () => trades().filter((t) => t.status !== "Completed");

  return (
    <div data-testid="active-orders">
      <div class="flex items-center justify-between mb-2">
        <label class="text-[11px] text-text-dim font-display uppercase tracking-widest">
          Active Orders
          <Show when={activeTrades().length > 0}>
            <span class="ml-1 text-text-secondary">({activeTrades().length})</span>
          </Show>
        </label>
        <button
          class="px-2 py-0.5 text-[10px] border-0 text-text-dim hover:text-text-secondary hover:bg-transparent"
          onClick={fetchTrades}
          title="Refresh"
          data-testid="refresh-orders"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
            <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
            <path d="M21 21v-5h-5" />
          </svg>
        </button>
      </div>

      <Show when={loading()}>
        <p class="text-[11px] text-text-dim font-mono py-2">Loading...</p>
      </Show>

      <Show when={error() && !loading()}>
        <p class="text-[11px] text-signal-red font-mono py-2" data-testid="orders-error">{error()}</p>
      </Show>

      <Show when={!loading() && !error() && activeTrades().length === 0}>
        <p class="text-[11px] text-text-dim font-mono py-2">No active orders</p>
      </Show>

      <div class="space-y-1">
        <For each={activeTrades()}>
          {(trade) => (
            <div
              class="flex items-center justify-between py-1.5 border-b border-border-grid last:border-0"
              data-testid="order-row"
            >
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-xs font-mono font-bold text-text-primary">
                    {trade.symbol}
                  </span>
                  <span
                    class={`text-[10px] font-display font-bold tracking-wider ${
                      trade.entry_quantity && parseFloat(trade.entry_quantity) > 0
                        ? "text-signal-green"
                        : "text-signal-red"
                    }`}
                  >
                    {trade.entry_quantity && parseFloat(trade.entry_quantity) > 0 ? "LONG" : "SHORT"}
                  </span>
                  <span class={`inline-block w-1.5 h-1.5 ${
                    trade.status === "Active" ? "bg-signal-green" :
                    trade.status === "Pending" ? "bg-signal-orange status-blink" :
                    "bg-text-dim"
                  }`} />
                  <span class="text-[10px] text-text-dim font-mono">{trade.status}</span>
                </div>
                <div class="flex gap-3 mt-0.5">
                  <Show when={trade.entry_price}>
                    <span class="text-[10px] text-text-dim font-mono">
                      E: {trade.entry_price}
                    </span>
                  </Show>
                  <Show when={trade.stop_loss_price}>
                    <span class="text-[10px] text-signal-red font-mono">
                      SL: {trade.stop_loss_price}
                    </span>
                  </Show>
                </div>
              </div>
              <button
                class="px-2 py-1 text-[9px] font-bold tracking-wider border-signal-red text-signal-red hover:bg-signal-red hover:text-text-primary ml-2"
                onClick={() => handleCancel(trade.id)}
                data-testid="cancel-order"
                title="Cancel trade"
              >
                X
              </button>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
