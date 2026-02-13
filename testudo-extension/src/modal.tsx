import { render } from "solid-js/web";
import { Show, For } from "solid-js";
import type { TradeSetup } from "./scraper";
import type { ManagementPreset, BalanceResponse } from "./types";
import { ORDER_EVENT_STYLES } from "./types";

export type ModalResult = "confirm" | "dismiss";

// --- Utility functions ---

function formatPrice(price: number): string {
  if (price >= 1000) return price.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (price >= 1) return price.toLocaleString("en-US", { minimumFractionDigits: 4, maximumFractionDigits: 4 });
  return price.toLocaleString("en-US", { minimumFractionDigits: 6, maximumFractionDigits: 8 });
}

function calculateRR(setup: TradeSetup): number {
  const risk = Math.abs(setup.entry - setup.stop);
  if (risk === 0) return 0;
  const reward = Math.abs(setup.target - setup.entry);
  return reward / risk;
}

// --- Styles (injected into Shadow DOM) ---

const MODAL_STYLES = `
  :host {
    all: initial;
    position: fixed;
    top: 0; left: 0;
    width: 100vw; height: 100vh;
    z-index: 99999;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: 'DM Sans', system-ui, -apple-system, sans-serif;
    -webkit-font-smoothing: antialiased;
  }
  .backdrop { position: absolute; inset: 0; background: rgba(0,0,0,0.5); backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px); }
  .panel {
    position: relative;
    background: rgba(21, 25, 33, 0.95);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 16px;
    padding: 22px 26px;
    min-width: 320px;
    max-width: 400px;
    box-shadow: 0 24px 48px rgba(0,0,0,0.4), 0 0 0 1px rgba(255,255,255,0.04);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
  }
  .panel.live-mode { border-color: rgba(239,68,68,0.3); box-shadow: 0 24px 48px rgba(239,68,68,0.15), 0 0 0 1px rgba(239,68,68,0.2); }
  .header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 18px; }
  .side { font-size: 12px; font-weight: 700; letter-spacing: 0.8px; text-transform: uppercase; padding: 5px 14px; border-radius: 20px; }
  .side.long { background: rgba(52,211,153,0.15); color: #34D399; }
  .side.short { background: rgba(248,113,113,0.15); color: #F87171; }
  .symbol { font-size: 14px; font-weight: 600; color: #fff; font-family: 'JetBrains Mono', ui-monospace, monospace; }
  .timeframe { font-size: 11px; color: #6B7280; margin-left: 8px; }
  .live-badge { display: inline-block; background: rgba(239,68,68,0.15); color: #EF4444; font-size: 10px; font-weight: 700; letter-spacing: 1px; padding: 3px 10px; border-radius: 20px; text-transform: uppercase; margin-bottom: 12px; }
  .live-warning { font-size: 11px; color: rgba(239,68,68,0.8); margin-bottom: 12px; text-align: center; }
  .rows { display: flex; flex-direction: column; gap: 10px; margin-bottom: 18px; }
  .row { display: flex; justify-content: space-between; align-items: center; }
  .label { font-size: 12px; color: #D4D4D8; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .value { font-size: 14px; font-family: 'JetBrains Mono', ui-monospace, monospace; color: #fff; font-weight: 500; }
  .divider { border: none; border-top: 1px solid rgba(255,255,255,0.08); margin: 14px 0; }
  .rr-row { display: flex; justify-content: space-between; align-items: center; }
  .rr-label { font-size: 13px; color: #D4D4D8; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .rr-value { font-size: 20px; font-weight: 700; font-family: 'JetBrains Mono', ui-monospace, monospace; letter-spacing: -0.5px; }
  .rr-value.good { color: #34D399; }
  .rr-value.bad { color: #F87171; }
  .rr-value.neutral { color: #FBBF24; }
  .mgmt-section { margin-top: 14px; padding-top: 14px; border-top: 1px solid rgba(255,255,255,0.08); }
  .mgmt-title { font-size: 11px; color: #A1A1AA; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 8px; font-weight: 600; }
  .mgmt-rule { font-size: 12px; color: #D4D4D8; padding: 3px 0; }
  .mgmt-rule .on { color: #34D399; font-weight: 600; }
  .mgmt-rule .off { color: #71717A; }
  .footer { display: flex; justify-content: space-between; align-items: center; margin-top: 18px; padding-top: 14px; border-top: 1px solid rgba(255,255,255,0.08); }
  .hint { font-size: 11px; color: #71717A; display: flex; align-items: center; gap: 4px; }
  kbd { display: inline-block; padding: 2px 7px; font-size: 10px; font-family: 'JetBrains Mono', ui-monospace, monospace; color: #D4D4D8; background: rgba(59,130,246,0.12); border: 1px solid rgba(59,130,246,0.25); border-radius: 6px; font-weight: 500; }
  .error-msg { color: #EF4444; font-size: 13px; text-align: center; padding: 20px 0; }
  .balance-section { margin-top: 14px; padding-top: 14px; border-top: 1px solid rgba(255,255,255,0.06); }
  .balance-row { display: flex; justify-content: space-between; align-items: center; padding: 3px 0; }
  .balance-label { font-size: 12px; color: #D4D4D8; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .balance-value { font-size: 14px; font-family: 'JetBrains Mono', ui-monospace, monospace; color: #34D399; font-weight: 500; }
  .balance-value.size { color: #fff; font-weight: 600; }
  .balance-value.leverage { color: #60A5FA; }
  .balance-value.margin { color: #FBBF24; }
  .balance-value.risk { color: #F87171; }
  .balance-value.muted { color: #71717A; font-style: italic; font-size: 12px; }
  .toast { position: fixed; top: 20px; right: 20px; padding: 12px 18px; font-size: 13px; font-weight: 600; z-index: 100000; opacity: 0; transition: opacity 0.3s; border-radius: 12px; box-shadow: 0 8px 24px rgba(0,0,0,0.3); }
  .toast.visible { opacity: 1; }
  .toast.success { background: rgba(34,197,94,0.15); color: #22C55E; border: 1px solid rgba(34,197,94,0.2); backdrop-filter: blur(12px); }
  .toast.error { background: rgba(239,68,68,0.15); color: #EF4444; border: 1px solid rgba(239,68,68,0.2); backdrop-filter: blur(12px); }
  .toast.info { background: rgba(59,130,246,0.15); color: #3B82F6; border: 1px solid rgba(59,130,246,0.2); backdrop-filter: blur(12px); }
`;

// --- Solid Components ---

function ManagementSummary(props: { preset: ManagementPreset }) {
  const rules = () => {
    const items: { label: string; value: string; active: boolean }[] = [
      { label: "Risk", value: `${props.preset.risk_percent}%`, active: true },
      { label: "Break-even", value: `at ${props.preset.break_even_at}%`, active: true },
      {
        label: "Trailing",
        value: props.preset.trailing_stop.enabled ? `${props.preset.trailing_stop.distance_percent}%` : "Off",
        active: props.preset.trailing_stop.enabled,
      },
      {
        label: "Partial TP",
        value: props.preset.partial_tp.enabled ? `${props.preset.partial_tp.close_percent}%` : "Off",
        active: props.preset.partial_tp.enabled,
      },
    ];
    return items;
  };

  return (
    <div class="mgmt-section">
      <div class="mgmt-title">Management Rules</div>
      <For each={rules()}>
        {(rule) => (
          <div class="mgmt-rule">
            <span class={rule.active ? "on" : "off"}>{rule.label}: {rule.value}</span>
          </div>
        )}
      </For>
    </div>
  );
}

function BalanceSummary(props: { balance: BalanceResponse[] | null; riskPercent: number; leverage: number; setup: TradeSetup }) {
  const usdt = () => props.balance?.find((b) => b.asset === "USDT");
  const available = () => {
    const b = usdt();
    return b ? parseFloat(b.available) : null;
  };
  const stopDistance = () => Math.abs(props.setup.entry - props.setup.stop);
  const riskAmount = () => {
    const avail = available();
    if (avail === null) return null;
    return (props.riskPercent / 100) * avail;
  };
  const positionSize = () => {
    const risk = riskAmount();
    const dist = stopDistance();
    if (risk === null || dist === 0) return null;
    return risk / dist;
  };
  const margin = () => {
    const qty = positionSize();
    if (qty === null) return null;
    return (qty * props.setup.entry) / props.leverage;
  };
  const baseAsset = () => props.setup.symbol.replace(/USDT$|USD$|BUSD$/, "");

  return (
    <div class="balance-section">
      <Show when={available() !== null} fallback={
        <div class="balance-row">
          <span class="balance-label">Balance</span>
          <span class="balance-value muted">unavailable</span>
        </div>
      }>
        <div class="balance-row">
          <span class="balance-label">Size</span>
          <span class="balance-value size">{positionSize()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 4 })} {baseAsset()}</span>
        </div>
        <div class="balance-row">
          <span class="balance-label">Leverage</span>
          <span class="balance-value leverage">{props.leverage}x</span>
        </div>
        <div class="balance-row">
          <span class="balance-label">Margin</span>
          <span class="balance-value margin">{margin()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDT</span>
        </div>
        <div class="balance-row">
          <span class="balance-label">Risk</span>
          <span class="balance-value risk">{riskAmount()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDT</span>
        </div>
        <div class="balance-row">
          <span class="balance-label">Available</span>
          <span class="balance-value">{available()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDT</span>
        </div>
      </Show>
    </div>
  );
}

function ConfirmationModal(props: {
  setup: TradeSetup | null;
  isLiveMode: boolean;
  management: ManagementPreset;
  balance: BalanceResponse[] | null;
  onResult: (result: ModalResult) => void;
}) {
  let enterCount = 0;

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      props.onResult("dismiss");
    } else if (e.key === "Enter" && props.setup) {
      e.preventDefault();
      e.stopPropagation();
      if (props.isLiveMode) {
        enterCount++;
        if (enterCount < 2) return;
      }
      props.onResult("confirm");
    }
  }

  document.addEventListener("keydown", handleKeyDown, true);

  return (
    <>
      <div class="backdrop" onClick={() => props.onResult("dismiss")} />
      <Show
        when={props.setup}
        fallback={
          <div class="panel">
            <div class="error-msg">No position tool detected</div>
            <div class="footer">
              <span class="hint"><kbd>Esc</kbd> dismiss</span>
            </div>
          </div>
        }
      >
        {(setup) => {
          const rr = calculateRR(setup());
          const rrClass = rr >= 2 ? "good" : rr >= 1 ? "neutral" : "bad";
          return (
            <div class={props.isLiveMode ? "panel live-mode" : "panel"}>
              <Show when={props.isLiveMode}>
                <span class="live-badge">LIVE MODE</span>
                <div class="live-warning">Real money trade. Press Enter twice to confirm.</div>
              </Show>
              <div class="header">
                <span class={`side ${setup().side.toLowerCase()}`}>{setup().side}</span>
                <span>
                  <span class="symbol">{setup().symbol}</span>
                  <span class="timeframe">{setup().timeframe}</span>
                </span>
              </div>
              <div class="rows">
                <div class="row">
                  <span class="label">Entry</span>
                  <span class="value">{formatPrice(setup().entry)}</span>
                </div>
                <div class="row">
                  <span class="label">Stop</span>
                  <span class="value">{formatPrice(setup().stop)}</span>
                </div>
                <div class="row">
                  <span class="label">Target</span>
                  <span class="value">{formatPrice(setup().target)}</span>
                </div>
              </div>
              <hr class="divider" />
              <div class="rr-row">
                <span class="rr-label">Risk : Reward</span>
                <span class={`rr-value ${rrClass}`}>1 : {rr.toFixed(2)}</span>
              </div>
              <ManagementSummary preset={props.management} />
              <BalanceSummary balance={props.balance} riskPercent={props.management.risk_percent} leverage={props.management.leverage} setup={setup()} />
              <div class="footer">
                <span class="hint">
                  {props.isLiveMode
                    ? <><kbd>Enter</kbd> <kbd>Enter</kbd> confirm</>
                    : <><kbd>Enter</kbd> execute</>}
                </span>
                <span class="hint"><kbd>Esc</kbd> dismiss</span>
              </div>
            </div>
          );
        }}
      </Show>
    </>
  );
}

// --- Modal lifecycle ---

let activeHost: HTMLElement | null = null;
let activeDispose: (() => void) | null = null;

export function showModal(
  setup: TradeSetup | null,
  isLiveMode: boolean,
  management: ManagementPreset,
  onResult: (result: ModalResult, setup: TradeSetup | null) => void,
  balance: BalanceResponse[] | null = null,
): void {
  dismiss();

  const host = document.createElement("div");
  host.id = "testudo-sniper-modal";
  const shadow = host.attachShadow({ mode: "closed" });

  const style = document.createElement("style");
  style.textContent = MODAL_STYLES;
  shadow.appendChild(style);

  const container = document.createElement("div");
  shadow.appendChild(container);

  const dispose = render(
    () => (
      <ConfirmationModal
        setup={setup}
        isLiveMode={isLiveMode}
        management={management}
        balance={balance}
        onResult={(result) => {
          dismiss();
          onResult(result, setup);
        }}
      />
    ),
    container,
  );

  document.body.appendChild(host);
  activeHost = host;
  activeDispose = dispose;
}

export function dismiss(): void {
  if (activeDispose) {
    activeDispose();
    activeDispose = null;
  }
  activeHost?.remove();
  activeHost = null;
}

export function isVisible(): boolean {
  return activeHost !== null;
}

// --- Toast Notifications ---

export type ToastStyle = "success" | "error" | "info";

export function showToast(message: string, type: ToastStyle = "success"): void {
  const host = document.createElement("div");
  host.id = "testudo-sniper-toast";
  const shadow = host.attachShadow({ mode: "closed" });

  const style = document.createElement("style");
  style.textContent = MODAL_STYLES;
  shadow.appendChild(style);

  const toast = document.createElement("div");
  toast.className = `toast ${type}`;
  toast.textContent = message;
  shadow.appendChild(toast);

  document.body.appendChild(host);

  requestAnimationFrame(() => toast.classList.add("visible"));

  setTimeout(() => {
    toast.classList.remove("visible");
    setTimeout(() => host.remove(), 300);
  }, 2000);
}

export function showOrderToast(eventType: string, message: string): void {
  const style = ORDER_EVENT_STYLES[eventType];
  showToast(message, style?.type || "success");
}
