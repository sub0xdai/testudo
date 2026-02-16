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
    overflow: hidden;
    background-color: rgba(21, 25, 33, 0.95);
    background-image:
      linear-gradient(to bottom, rgba(21, 25, 33, 0.4) 0%, rgba(21, 25, 33, 0.95) 65%),
      url(data:image/jpeg;base64,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);
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 16px;
    padding: 22px 26px;
    min-width: 320px;
    max-width: 400px;
    box-shadow: 0 24px 48px rgba(0,0,0,0.4), 0 0 0 1px rgba(255,255,255,0.04);
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
