import { render } from "solid-js/web";
import TradeForm from "./components/TradeForm";
import type { TradeSetup } from "./scraper";
import type { ManagementPreset, BalanceResponse } from "./types";
import { ORDER_EVENT_STYLES } from "./types";

export type ModalResult = "confirm" | "dismiss";

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
    --color-signal-green: #4ade80;
    --color-signal-red: #ef4444;
    --color-signal-orange: #f59e0b;
    --color-text-primary: #ffffff;
    --color-text-secondary: #9ca3af;
    --color-text-dim: #6b7280;
    --color-accent-steel: #94a3b8;
    --color-bg-core: #0b0e11;
    --color-bg-panel: #141920;
    --color-bg-elevated: #1c2128;
  }
  .backdrop { position: absolute; inset: 0; background: rgba(0,0,0,0.5); backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px); }
  .panel {
    position: relative;
    overflow: hidden;
    background-color: rgba(21, 25, 33, 0.95);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 16px;
    padding: 22px 26px;
    min-width: 320px;
    max-width: 400px;
    box-shadow: 0 24px 48px rgba(0,0,0,0.4), 0 0 0 1px rgba(255,255,255,0.04);
  }
  .panel.live-mode { border-color: color-mix(in srgb, var(--color-signal-red) 30%, transparent); box-shadow: 0 24px 48px color-mix(in srgb, var(--color-signal-red) 15%, transparent), 0 0 0 1px color-mix(in srgb, var(--color-signal-red) 20%, transparent); }
  .header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 18px; gap: 12px; }
  .side-toggle { display: flex; gap: 4px; }
  .side-btn {
    font-size: 11px; font-weight: 700; letter-spacing: 0.8px; text-transform: uppercase;
    padding: 5px 14px; border-radius: 20px; border: 1px solid rgba(255,255,255,0.1);
    background: transparent; color: var(--color-text-dim); cursor: pointer; transition: all 0.15s;
  }
  .side-btn:hover { border-color: rgba(255,255,255,0.2); color: var(--color-text-secondary); }
  .side-btn-active-long { background: color-mix(in srgb, var(--color-signal-green) 15%, transparent); color: var(--color-signal-green); border-color: color-mix(in srgb, var(--color-signal-green) 30%, transparent); }
  .side-btn-active-short { background: color-mix(in srgb, var(--color-signal-red) 15%, transparent); color: var(--color-signal-red); border-color: color-mix(in srgb, var(--color-signal-red) 30%, transparent); }
  .symbol-field { position: relative; flex: 1; }
  .field-input {
    width: 100%;
    background: rgba(255,255,255,0.05);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 8px;
    padding: 8px 12px;
    font-size: 14px;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    color: #fff;
    outline: none;
    transition: border-color 0.15s;
    box-sizing: border-box;
  }
  .field-input-sm { font-size: 13px; padding: 6px 10px; }
  .field-input:focus { border-color: color-mix(in srgb, var(--color-accent-steel) 50%, transparent); }
  .field-input.invalid { border-color: rgba(239,68,68,0.5); }
  .field-input.auto-filled { border-color: color-mix(in srgb, var(--color-signal-green) 30%, transparent); }
  .field-input::placeholder { color: var(--color-text-dim); }
  .auto-badge {
    position: absolute; right: 8px; top: 50%; transform: translateY(-50%);
    font-size: 9px; font-weight: 700; letter-spacing: 0.5px; text-transform: uppercase;
    color: var(--color-signal-green); background: color-mix(in srgb, var(--color-signal-green) 10%, transparent); padding: 2px 6px; border-radius: 4px;
    cursor: pointer; transition: opacity 0.15s;
  }
  .auto-badge:hover { opacity: 0.6; }
  .rows { display: flex; flex-direction: column; gap: 10px; margin-bottom: 18px; }
  .field-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .field-wrapper { position: relative; flex: 1; }
  .label { font-size: 12px; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; min-width: 50px; }
  .value { font-size: 14px; font-family: 'JetBrains Mono', ui-monospace, monospace; color: #fff; font-weight: 500; }
  .divider { border: none; border-top: 1px solid rgba(255,255,255,0.08); margin: 14px 0; }
  .rr-row { display: flex; justify-content: space-between; align-items: center; }
  .rr-label { font-size: 13px; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .rr-value { font-size: 20px; font-weight: 700; font-family: 'JetBrains Mono', ui-monospace, monospace; letter-spacing: -0.5px; }
  .rr-value.good { color: var(--color-signal-green); }
  .rr-value.bad { color: var(--color-signal-red); }
  .rr-value.neutral { color: var(--color-signal-orange); }
  .mgmt-section { margin-top: 14px; padding-top: 14px; border-top: 1px solid rgba(255,255,255,0.08); }
  .mgmt-title { font-size: 11px; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 1px; margin-bottom: 8px; font-weight: 600; }
  .mgmt-rule { font-size: 12px; color: var(--color-text-secondary); padding: 3px 0; }
  .mgmt-rule .on { color: var(--color-signal-green); font-weight: 600; }
  .mgmt-rule .off { color: var(--color-text-dim); }
  .footer { display: flex; justify-content: space-between; align-items: center; margin-top: 18px; padding-top: 14px; border-top: 1px solid rgba(255,255,255,0.08); }
  .hint { font-size: 11px; color: var(--color-text-dim); display: flex; align-items: center; gap: 4px; }
  kbd { display: inline-block; padding: 2px 7px; font-size: 10px; font-family: 'JetBrains Mono', ui-monospace, monospace; color: var(--color-text-secondary); background: color-mix(in srgb, var(--color-accent-steel) 12%, transparent); border: 1px solid color-mix(in srgb, var(--color-accent-steel) 25%, transparent); border-radius: 6px; font-weight: 500; }
  .live-badge { display: inline-block; background: color-mix(in srgb, var(--color-signal-red) 15%, transparent); color: var(--color-signal-red); font-size: 10px; font-weight: 700; letter-spacing: 1px; padding: 3px 10px; border-radius: 20px; text-transform: uppercase; margin-bottom: 12px; }
  .live-warning { font-size: 11px; color: rgba(239,68,68,0.8); margin-bottom: 12px; text-align: center; }
  .balance-section { margin-top: 14px; padding-top: 14px; border-top: 1px solid rgba(255,255,255,0.06); }
  .balance-row { display: flex; justify-content: space-between; align-items: center; padding: 3px 0; }
  .balance-label { font-size: 12px; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .balance-value { font-size: 14px; font-family: 'JetBrains Mono', ui-monospace, monospace; color: var(--color-signal-green); font-weight: 500; }
  .balance-value.size { color: #fff; font-weight: 600; }
  .balance-value.leverage { color: var(--color-accent-steel); }
  .balance-value.margin { color: var(--color-signal-orange); }
  .balance-value.risk { color: var(--color-signal-red); }
  .balance-value.muted { color: var(--color-text-dim); font-style: italic; font-size: 12px; }
  .toast { position: fixed; top: 20px; right: 20px; padding: 12px 18px; font-size: 13px; font-weight: 600; z-index: 100000; opacity: 0; transition: opacity 0.3s; border-radius: 12px; box-shadow: 0 8px 24px rgba(0,0,0,0.3); }
  .toast.visible { opacity: 1; }
  .toast.success { background: #052E16; color: var(--color-signal-green); border: 1px solid color-mix(in srgb, var(--color-signal-green) 30%, black); }
  .toast.error { background: #450A0A; color: var(--color-signal-red); border: 1px solid color-mix(in srgb, var(--color-signal-red) 30%, black); }
  .toast.info { background: #0F172A; color: var(--color-text-secondary); border: 1px solid #334155; }
`;

// --- Modal lifecycle ---

let activeHost: HTMLElement | null = null;
let activeDispose: (() => void) | null = null;

export function showModal(
  setup: TradeSetup | null,
  management: ManagementPreset,
  onResult: (result: ModalResult, setup: TradeSetup | null) => void,
  balance: BalanceResponse[] | null = null,
  activeExchange: string | null = null,
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
      <>
        <div class="backdrop" onClick={() => { dismiss(); onResult("dismiss", null); }} />
        <TradeForm
          initialSetup={setup}
          management={management}
          balance={balance}
          activeExchange={activeExchange}
          onConfirm={(editedSetup) => {
            dismiss();
            onResult("confirm", editedSetup);
          }}
          onDismiss={() => {
            dismiss();
            onResult("dismiss", null);
          }}
        />
      </>
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

const activeToasts: HTMLElement[] = [];
const MAX_TOASTS = 3;

export function showToast(message: string, type: ToastStyle = "success"): void {
  // Evict oldest if at cap
  while (activeToasts.length >= MAX_TOASTS) {
    const oldest = activeToasts.shift();
    oldest?.remove();
  }

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
  activeToasts.push(host);

  requestAnimationFrame(() => toast.classList.add("visible"));

  setTimeout(() => {
    toast.classList.remove("visible");
    setTimeout(() => {
      host.remove();
      const idx = activeToasts.indexOf(host);
      if (idx !== -1) activeToasts.splice(idx, 1);
    }, 300);
  }, 2000);
}

export function showOrderToast(eventType: string, message: string): void {
  const style = ORDER_EVENT_STYLES[eventType];
  showToast(message, style?.type || "success");
}
