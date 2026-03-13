import { Show, For } from "solid-js";
import type { TradeGroupResponse } from "../../types";

interface PositionCardProps {
  trade: TradeGroupResponse;
  onCancel: (tradeId: string) => void;
  cancelling?: boolean;
}

function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  return `${days}d ago`;
}

export default function PositionCard(props: PositionCardProps) {
  const isLong = () => {
    const entry = parseFloat(props.trade.entry_price || "0");
    const sl = parseFloat(props.trade.stop_loss_price || "0");
    if (entry > 0 && sl > 0) return sl < entry;
    // Fallback to quantity sign if prices unavailable
    return parseFloat(props.trade.entry_quantity) > 0;
  };
  const direction = () => isLong() ? "LONG" : "SHORT";

  const statusClass = () => {
    switch (props.trade.status) {
      case "Active": return "text-signal-green bg-signal-green/10";
      case "Pending": return "text-signal-orange bg-signal-orange/10 status-blink";
      default: return "text-text-dim bg-bg-elevated";
    }
  };

  const beLabel = () => {
    if (!props.trade.break_even_enabled) return "BE: OFF";
    if (props.trade.break_even_triggered) return "BE: triggered";
    return "BE: armed";
  };

  const beClass = () => {
    if (!props.trade.break_even_enabled) return "text-text-dim border-border-subtle";
    if (props.trade.break_even_triggered) return "text-signal-green border-signal-green/30";
    return "text-signal-orange border-signal-orange/30";
  };

  return (
    <div
      class={`bg-bg-panel border border-border-subtle rounded-xl p-4 card-depth ${isLong() ? "accent-long" : "accent-short"}`}
      data-testid="position-card"
    >
      {/* Row 1: Symbol + Direction + Status */}
      <div class="flex items-center justify-between mb-2.5">
        <div class="flex items-center gap-2.5">
          <span class="text-[14px] font-mono font-bold text-white truncate max-w-[120px]" data-testid="position-symbol">
            {props.trade.symbol}
          </span>
          <span
            class={`text-[10px] font-bold tracking-wider font-sans ${isLong() ? "text-signal-green" : "text-signal-red"}`}
            data-testid="position-direction"
          >
            {direction()}
          </span>
        </div>
        <span
          class={`text-[9px] uppercase tracking-wider px-2.5 py-0.5 rounded-full font-sans font-medium ${statusClass()}`}
          data-testid="position-status"
        >
          {props.trade.status}
        </span>
      </div>

      {/* Timestamp */}
      <Show when={props.trade.created_at}>
        <div class="text-[9px] text-text-dim font-sans mb-1.5">{formatRelativeTime(props.trade.created_at)}</div>
      </Show>

      {/* Row 2: Entry + SL */}
      <div class="flex gap-6 mb-1.5">
        <div data-testid="position-entry">
          <span class="text-[10px] text-text-dim font-sans">Entry </span>
          <span class="text-[12px] text-text-secondary font-mono">
            {props.trade.entry_price || "--"}
          </span>
        </div>
        <Show when={props.trade.stop_loss_price}>
          <div data-testid="position-sl">
            <span class="text-[10px] text-signal-red font-sans">SL </span>
            <span class="text-[12px] text-signal-red font-mono">
              {props.trade.stop_loss_price}
            </span>
          </div>
        </Show>
      </div>

      {/* Row 3: TP targets */}
      <Show when={props.trade.take_profit_targets.length > 0}>
        <div class="mb-2.5">
          <For each={props.trade.take_profit_targets}>
            {(tp, i) => (
              <div class="flex items-center gap-2" data-testid="position-tp">
                <span class="text-[10px] text-text-dim font-sans">TP{i() + 1}</span>
                <span class="text-[11px] text-text-secondary font-mono">{tp.price}</span>
                <span class="text-[9px] text-text-dim font-mono">({tp.percent_to_close}%)</span>
                <span class={`text-[9px] font-sans font-medium px-1.5 py-px rounded-full ${
                  tp.filled ? "text-signal-green bg-signal-green/10" : "text-text-dim bg-bg-elevated"
                }`}>
                  {tp.filled ? "filled" : "pending"}
                </span>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Row 4: Management badges + Cancel */}
      <div class="flex items-center justify-between mt-3 pt-2.5 border-t border-border-subtle">
        <div class="flex gap-2">
          <span
            class={`text-[9px] uppercase font-sans font-medium px-2 py-0.5 rounded-full border ${beClass()}`}
            data-testid="position-be-badge"
          >
            {beLabel()}
          </span>
          <span
            class="text-[9px] uppercase font-sans font-medium px-2 py-0.5 rounded-full border text-text-dim border-border-subtle"
            data-testid="position-trail-badge"
          >
            Trail: {props.trade.trailing_stop_enabled === true ? "ON" : "OFF"}
          </span>
        </div>
        <button
          class={`px-4 py-2.5 min-h-[44px] text-xs font-bold tracking-wider font-sans rounded-full border-signal-red ${
            props.cancelling ? "opacity-50 cursor-wait" : "text-signal-red hover:bg-signal-red hover:text-white"
          }`}
          onClick={() => props.onCancel(props.trade.id)}
          disabled={props.cancelling}
          data-testid="cancel-order"
          title="Cancel trade"
        >
          {props.cancelling ? "..." : "CANCEL"}
        </button>
      </div>
    </div>
  );
}
