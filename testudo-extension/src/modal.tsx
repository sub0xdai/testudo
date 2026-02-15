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
  .header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 18px; gap: 12px; }
  .side-toggle { display: flex; gap: 4px; }
  .side-btn {
    font-size: 11px; font-weight: 700; letter-spacing: 0.8px; text-transform: uppercase;
    padding: 5px 14px; border-radius: 20px; border: 1px solid rgba(255,255,255,0.1);
    background: transparent; color: #71717A; cursor: pointer; transition: all 0.15s;
  }
  .side-btn:hover { border-color: rgba(255,255,255,0.2); color: #D4D4D8; }
  .side-btn-active-long { background: rgba(52,211,153,0.15); color: #34D399; border-color: rgba(52,211,153,0.3); }
  .side-btn-active-short { background: rgba(248,113,113,0.15); color: #F87171; border-color: rgba(248,113,113,0.3); }
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
  .field-input:focus { border-color: rgba(59,130,246,0.5); }
  .field-input.invalid { border-color: rgba(239,68,68,0.5); }
  .field-input.auto-filled { border-color: rgba(52,211,153,0.3); }
  .field-input::placeholder { color: #52525B; }
  .auto-badge {
    position: absolute; right: 8px; top: 50%; transform: translateY(-50%);
    font-size: 9px; font-weight: 700; letter-spacing: 0.5px; text-transform: uppercase;
    color: #34D399; background: rgba(52,211,153,0.1); padding: 2px 6px; border-radius: 4px;
    cursor: pointer; transition: opacity 0.15s;
  }
  .auto-badge:hover { opacity: 0.6; }
  .rows { display: flex; flex-direction: column; gap: 10px; margin-bottom: 18px; }
  .field-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .field-wrapper { position: relative; flex: 1; }
  .label { font-size: 12px; color: #D4D4D8; text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; min-width: 50px; }
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
  .live-badge { display: inline-block; background: rgba(239,68,68,0.15); color: #EF4444; font-size: 10px; font-weight: 700; letter-spacing: 1px; padding: 3px 10px; border-radius: 20px; text-transform: uppercase; margin-bottom: 12px; }
  .live-warning { font-size: 11px; color: rgba(239,68,68,0.8); margin-bottom: 12px; text-align: center; }
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
      <>
        <div class="backdrop" onClick={() => { dismiss(); onResult("dismiss", null); }} />
        <TradeForm
          initialSetup={setup}
          isLiveMode={isLiveMode}
          management={management}
          balance={balance}
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
