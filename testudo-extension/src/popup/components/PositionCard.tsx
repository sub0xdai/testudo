import { Show, For } from "solid-js";
import type { TradeGroupResponse } from "../../types";

interface PositionCardProps {
  trade: TradeGroupResponse;
  onCancel: (tradeId: string) => void;
}

export default function PositionCard(props: PositionCardProps) {
  const isLong = () => props.trade.entry_quantity && parseFloat(props.trade.entry_quantity) > 0;
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
    if (!props.trade.break_even_enabled) return "text-text-dim border-border-grid";
    if (props.trade.break_even_triggered) return "text-signal-green border-signal-green";
    return "text-signal-orange border-signal-orange";
  };

  return (
    <div
      class={`bg-bg-panel border border-border-grid p-4 card-depth ${isLong() ? "accent-long" : "accent-short"}`}
      data-testid="position-card"
    >
      {/* Row 1: Symbol + Direction + Status */}
      <div class="flex items-center justify-between mb-2.5">
        <div class="flex items-center gap-2.5">
          <span class="text-[14px] font-mono font-bold text-text-primary" data-testid="position-symbol">
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
          class={`text-[9px] uppercase tracking-wider px-2 py-0.5 font-sans ${statusClass()}`}
          data-testid="position-status"
        >
          {props.trade.status}
        </span>
      </div>

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
                <span class={`text-[9px] font-mono ${tp.filled ? "text-signal-green" : "text-text-dim"}`}>
                  [{tp.filled ? "filled" : "pending"}]
                </span>
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* Row 4: Management badges + Cancel */}
      <div class="flex items-center justify-between mt-3 pt-2.5 border-t border-border-grid">
        <div class="flex gap-2">
          <span
            class={`text-[9px] uppercase font-mono px-1.5 py-0.5 border ${beClass()}`}
            data-testid="position-be-badge"
          >
            {beLabel()}
          </span>
          <span
            class="text-[9px] uppercase font-mono px-1.5 py-0.5 border text-text-dim border-border-grid"
            data-testid="position-trail-badge"
          >
            Trail: OFF
          </span>
        </div>
        <button
          class="px-2.5 py-1 text-[9px] font-bold tracking-wider font-sans border-signal-red text-signal-red hover:bg-signal-red hover:text-text-primary"
          onClick={() => props.onCancel(props.trade.id)}
          data-testid="cancel-order"
          title="Cancel trade"
        >
          CANCEL
        </button>
      </div>
    </div>
  );
}
