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
    font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  }
  .backdrop { position: absolute; inset: 0; background: rgba(0,0,0,0.6); }
  .panel { position: relative; background: #1a1a2e; border: 1px solid #333; padding: 20px 24px; min-width: 320px; max-width: 400px; box-shadow: 0 8px 32px rgba(0,0,0,0.5); }
  .panel.live-mode { border-color: #ff4757; box-shadow: 0 8px 32px rgba(255,71,87,0.3); }
  .header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
  .side { font-size: 13px; font-weight: 700; letter-spacing: 1px; text-transform: uppercase; padding: 4px 10px; }
  .side.long { background: #00d4aa; color: #1a1a2e; }
  .side.short { background: #ff4757; color: #fff; }
  .symbol { font-size: 15px; font-weight: 600; color: #e0e0e0; font-family: monospace; }
  .timeframe { font-size: 11px; color: #666; margin-left: 8px; }
  .live-badge { display: inline-block; background: #ff4757; color: #fff; font-size: 10px; font-weight: 700; letter-spacing: 1px; padding: 2px 8px; text-transform: uppercase; margin-bottom: 12px; }
  .live-warning { font-size: 11px; color: #ff4757; margin-bottom: 12px; text-align: center; }
  .rows { display: flex; flex-direction: column; gap: 8px; margin-bottom: 16px; }
  .row { display: flex; justify-content: space-between; align-items: center; }
  .label { font-size: 11px; color: #888; text-transform: uppercase; letter-spacing: 0.5px; }
  .value { font-size: 14px; font-family: monospace; color: #e0e0e0; }
  .divider { border: none; border-top: 1px solid #333; margin: 12px 0; }
  .rr-row { display: flex; justify-content: space-between; align-items: center; }
  .rr-label { font-size: 12px; color: #888; text-transform: uppercase; letter-spacing: 0.5px; }
  .rr-value { font-size: 18px; font-weight: 700; font-family: monospace; }
  .rr-value.good { color: #00d4aa; }
  .rr-value.bad { color: #ff4757; }
  .rr-value.neutral { color: #ffa502; }
  .mgmt-section { margin-top: 12px; padding-top: 12px; border-top: 1px solid #333; }
  .mgmt-title { font-size: 10px; color: #666; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 8px; }
  .mgmt-rule { font-size: 11px; color: #aaa; padding: 2px 0; }
  .mgmt-rule .on { color: #00d4aa; }
  .mgmt-rule .off { color: #555; }
  .footer { display: flex; justify-content: space-between; align-items: center; margin-top: 16px; padding-top: 12px; border-top: 1px solid #333; }
  .hint { font-size: 11px; color: #555; }
  kbd { display: inline-block; padding: 1px 5px; font-size: 11px; font-family: monospace; color: #aaa; background: #16213e; border: 1px solid #444; }
  .error-msg { color: #ff4757; font-size: 13px; text-align: center; padding: 20px 0; }
  .balance-section { margin-top: 12px; padding-top: 12px; border-top: 1px solid #333; }
  .balance-row { display: flex; justify-content: space-between; align-items: center; padding: 2px 0; }
  .balance-label { font-size: 11px; color: #888; text-transform: uppercase; letter-spacing: 0.5px; }
  .balance-value { font-size: 14px; font-family: monospace; color: #00d4aa; }
  .balance-value.margin { color: #ffa502; }
  .balance-value.muted { color: #555; font-style: italic; font-size: 12px; }
  .toast { position: fixed; top: 20px; right: 20px; padding: 10px 16px; font-size: 13px; font-weight: 500; z-index: 100000; opacity: 0; transition: opacity 0.3s; }
  .toast.visible { opacity: 1; }
  .toast.success { background: #00d4aa; color: #1a1a2e; }
  .toast.error { background: #ff4757; color: #fff; }
  .toast.info { background: #2196f3; color: #fff; }
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

function BalanceSummary(props: { balance: BalanceResponse[] | null; riskPercent: number }) {
  const usdt = () => props.balance?.find((b) => b.asset === "USDT");
  const available = () => {
    const b = usdt();
    return b ? parseFloat(b.available) : null;
  };
  const margin = () => {
    const avail = available();
    if (avail === null) return null;
    return (props.riskPercent / 100) * avail;
  };

  return (
    <div class="balance-section">
      <Show when={available() !== null} fallback={
        <div class="balance-row">
          <span class="balance-label">Balance</span>
          <span class="balance-value muted">unavailable</span>
        </div>
      }>
        <div class="balance-row">
          <span class="balance-label">Available</span>
          <span class="balance-value">{available()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDT</span>
        </div>
        <div class="balance-row">
          <span class="balance-label">Margin</span>
          <span class="balance-value margin">~{margin()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDT</span>
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
              <BalanceSummary balance={props.balance} riskPercent={props.management.risk_percent} />
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
