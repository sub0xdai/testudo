/** @anchor api:ext:modal
 * @tags api */

import { render } from "solid-js/web";
import TradeForm from "./components/TradeForm";
import type { TradeSetup } from "./scraper";
import type { ManagementPreset, BalanceResponse } from "./types";
import { ORDER_EVENT_STYLES } from "./types";

export type ModalResult = "confirm" | "dismiss";

// --- Shared toast CSS (used by both modal Shadow DOM and standalone toasts) ---

const TOAST_CSS = `
  .toast { position: fixed; top: 20px; right: 20px; padding: 10px 16px; font-size: 12px; font-weight: 600; font-family: 'Space Mono', monospace; letter-spacing: 0.05em; z-index: 100000; opacity: 0; transition: opacity 0.3s; border-radius: 0; box-shadow: 0 8px 24px rgba(0,0,0,0.5); display: flex; align-items: center; gap: 8px; }
  .toast.visible { opacity: 1; }
  .toast .icon { font-size: 14px; flex-shrink: 0; }
  .toast.success { background: #090a0d; color: #22c55e; border: 1px solid #1a3a1a; }
  .toast.success .icon::before { content: "\\2713"; }
  .toast.error { background: #090a0d; color: #ef4444; border: 1px solid #3a1a1a; }
  .toast.error .icon::before { content: "\\2717"; }
  .toast.info { background: #090a0d; color: #b9bec8; border: 1px solid #2d303a; }
  .toast.info .icon::before { content: "\\2022"; }
  .testudo-banner { position: fixed; top: 12px; left: 50%; transform: translateX(-50%); z-index: 2147483647; font-family: 'Space Mono', monospace; font-size: 12px; padding: 10px 16px; display: flex; align-items: center; gap: 10px; max-width: 500px; opacity: 0; transition: opacity 0.3s; }
  .testudo-banner.visible { opacity: 1; }
  .testudo-banner.error { background: #090a0d; color: #f59e0b; border: 1px solid #3a2a0a; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
  .testudo-banner .icon { font-size: 14px; flex-shrink: 0; }
  .testudo-banner .icon::before { content: "\\26A0"; }
  .testudo-banner .message { flex: 1; }
  .testudo-banner .action { color: #d4d4d4; text-decoration: underline; cursor: pointer; white-space: nowrap; }
  .testudo-banner .action:hover { color: #fff; }
  .testudo-banner .dismiss { background: none; border: none; color: #737882; cursor: pointer; font-size: 16px; padding: 0 4px; line-height: 1; }
  .testudo-banner .dismiss:hover { color: #fff; }
`;

// --- Styles (injected into Shadow DOM) ---

const fontBaseUrl = typeof chrome !== "undefined" && chrome.runtime?.getURL
  ? chrome.runtime.getURL("popup/fonts/")
  : "./fonts/";

const MODAL_STYLES = `
  @font-face {
    font-family: 'Space Grotesk';
    src: url('${fontBaseUrl}space-grotesk-variable.woff2') format('woff2');
    font-weight: 300 700;
    font-display: swap;
  }
  @font-face {
    font-family: 'Space Mono';
    src: url('${fontBaseUrl}space-mono-regular.woff2') format('woff2');
    font-weight: 400;
    font-display: swap;
  }
  @font-face {
    font-family: 'Space Mono';
    src: url('${fontBaseUrl}space-mono-bold.woff2') format('woff2');
    font-weight: 700;
    font-display: swap;
  }
  :host {
    all: initial;
    position: fixed;
    top: 0; left: 0;
    width: 100vw; height: 100vh;
    z-index: 99999;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: 'Space Grotesk', system-ui, -apple-system, sans-serif;
    -webkit-font-smoothing: antialiased;
    --color-signal-green: #22C55E;
    --color-signal-red: #EF4444;
    --color-signal-orange: #f59e0b;
    --color-text-primary: #ededed;
    --color-text-secondary: #b9bec8;
    --color-text-dim: #737882;
    --color-accent-steel: #94a3b8;
    --color-accent-primary: #94a3b8;
    --color-bg-core: #090a0d;
    --color-bg-panel: #13151a;
    --color-bg-elevated: #1a1c23;
    --color-border: #2d303a;
  }
  :host([data-theme="light"]) {
    --color-signal-green: #05963c;
    --color-signal-red: #c81419;
    --color-signal-orange: #b46e00;
    --color-text-primary: #0a0c14;
    --color-text-secondary: #373e4e;
    --color-text-dim: #5f6676;
    --color-accent-steel: #374151;
    --color-accent-primary: #374151;
    --color-bg-core: #f1f3f7;
    --color-bg-panel: #ffffff;
    --color-bg-elevated: #eaecf2;
    --color-border: #c3c8d4;
  }
  .backdrop { position: absolute; inset: 0; background: rgba(0,0,0,0.5); }
  .panel {
    position: relative;
    overflow: hidden;
    background-color: color-mix(in srgb, var(--color-bg-panel) 95%, transparent);
    border: 1px solid var(--color-border);
    border-radius: 0;
    padding: 28px 32px;
    min-width: 380px;
    max-width: 480px;
    box-shadow: 0 24px 48px rgba(0,0,0,0.5);
  }
  .panel.live-mode { border-color: color-mix(in srgb, var(--color-signal-red) 40%, var(--color-border)); box-shadow: 0 24px 48px color-mix(in srgb, var(--color-signal-red) 15%, transparent); }
  .header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 18px; gap: 12px; }
  .side-toggle { display: flex; gap: 4px; }
  .side-btn {
    font-size: 12px; font-weight: 700; letter-spacing: 0.8px; text-transform: uppercase;
    padding: 7px 18px; border-radius: 0; border: 1px solid var(--color-border);
    background: transparent; color: var(--color-text-dim); cursor: pointer; transition: background-color 0.15s, color 0.15s, border-color 0.15s;
    font-family: 'Space Mono', ui-monospace, monospace;
  }
  .side-btn:hover { border-color: #52525B; color: var(--color-text-secondary); }
  .side-btn-active-long { background: color-mix(in srgb, var(--color-signal-green) 15%, transparent); color: var(--color-signal-green); border-color: color-mix(in srgb, var(--color-signal-green) 30%, transparent); }
  .side-btn-active-short { background: color-mix(in srgb, var(--color-signal-red) 15%, transparent); color: var(--color-signal-red); border-color: color-mix(in srgb, var(--color-signal-red) 30%, transparent); }
  .symbol-field { position: relative; flex: 1; }
  .field-input {
    width: 100%;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border);
    border-radius: 0;
    padding: 8px 12px;
    font-size: 14px;
    font-family: 'Space Mono', ui-monospace, monospace;
    color: var(--color-text-primary);
    outline: none;
    transition: border-color 0.15s;
    box-sizing: border-box;
  }
  .field-input-sm { font-size: 13px; padding: 6px 10px; }
  .field-input:focus { border-color: color-mix(in srgb, var(--color-accent-steel) 50%, transparent); }
  .field-input.invalid { border-color: color-mix(in srgb, var(--color-signal-red) 50%, transparent); }
  .field-input.auto-filled { border-color: color-mix(in srgb, var(--color-signal-green) 30%, transparent); }
  .field-input::placeholder { color: var(--color-text-dim); }
  .auto-badge {
    position: absolute; right: 8px; top: 50%; transform: translateY(-50%);
    font-size: 9px; font-weight: 700; letter-spacing: 0.5px; text-transform: uppercase;
    color: var(--color-signal-green); background: color-mix(in srgb, var(--color-signal-green) 10%, transparent); padding: 2px 6px; border-radius: 0;
    cursor: pointer; transition: opacity 0.15s;
  }
  .auto-badge:hover { opacity: 0.6; }
  .rows { display: flex; flex-direction: column; gap: 10px; margin-bottom: 18px; }
  .field-row { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .field-wrapper { position: relative; flex: 1; }
  .suggestions {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    max-height: 180px;
    overflow-y: auto;
    background: var(--color-bg-elevated);
    border: 1px solid var(--color-border);
    border-radius: 0;
    list-style: none;
    margin: 0;
    padding: 0;
    z-index: 10;
    box-shadow: 0 8px 16px rgba(0,0,0,0.4);
  }
  .suggestion-item {
    padding: 6px 10px;
    font-size: 12px;
    font-family: 'Space Mono', ui-monospace, monospace;
    color: var(--color-text-secondary);
    cursor: pointer;
    user-select: none;
  }
  .suggestion-item.highlighted {
    background: color-mix(in srgb, var(--color-accent-steel) 18%, transparent);
    color: var(--color-text-primary);
  }
  .label { font-size: 12px; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; min-width: 50px; }
  .value { font-size: 14px; font-family: 'Space Mono', ui-monospace, monospace; color: var(--color-text-primary); font-weight: 500; }
  .divider { border: none; border-top: 1px solid var(--color-border); margin: 14px 0; }
  .rr-row { display: flex; justify-content: space-between; align-items: center; }
  .rr-label { font-size: 13px; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .rr-value { font-size: 20px; font-weight: 700; font-family: 'Space Mono', ui-monospace, monospace; letter-spacing: -0.5px; }
  .rr-value.good { color: var(--color-signal-green); }
  .rr-value.bad { color: var(--color-signal-red); }
  .rr-value.neutral { color: var(--color-signal-orange); }
  .mgmt-section { margin-top: 14px; padding-top: 14px; border-top: 1px solid var(--color-border); }
  .mgmt-title { font-size: 11px; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 1px; margin-bottom: 8px; font-weight: 600; }
  .mgmt-rule { font-size: 12px; color: var(--color-text-secondary); padding: 3px 0; }
  .mgmt-rule .on { color: var(--color-signal-green); font-weight: 600; }
  .mgmt-rule .off { color: var(--color-text-dim); }
  .footer { display: flex; justify-content: space-between; align-items: center; margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--color-border); }
  .hint { font-size: 11px; color: var(--color-text-dim); display: flex; align-items: center; gap: 4px; }
  kbd { display: inline-block; padding: 2px 7px; font-size: 10px; font-family: 'Space Mono', ui-monospace, monospace; color: var(--color-text-secondary); background: color-mix(in srgb, var(--color-accent-steel) 12%, transparent); border: 1px solid color-mix(in srgb, var(--color-accent-steel) 25%, transparent); border-radius: 0; font-weight: 500; }
  .live-badge { display: inline-block; background: color-mix(in srgb, var(--color-signal-red) 15%, transparent); color: var(--color-signal-red); font-size: 10px; font-weight: 700; letter-spacing: 1px; padding: 3px 10px; border-radius: 0; text-transform: uppercase; margin-bottom: 12px; font-family: 'Space Mono', ui-monospace, monospace; }
  .live-warning { font-size: 11px; color: color-mix(in srgb, var(--color-signal-red) 80%, transparent); margin-bottom: 12px; text-align: center; }
  .balance-section { margin-top: 14px; padding-top: 14px; border-top: 1px solid var(--color-border); }
  .balance-row { display: flex; justify-content: space-between; align-items: center; padding: 3px 0; }
  .balance-label { font-size: 12px; color: var(--color-text-secondary); text-transform: uppercase; letter-spacing: 0.5px; font-weight: 600; }
  .balance-value { font-size: 14px; font-family: 'Space Mono', ui-monospace, monospace; color: var(--color-signal-green); font-weight: 500; }
  .balance-value.size { color: var(--color-text-primary); font-weight: 600; }
  .balance-value.leverage { color: var(--color-accent-steel); }
  .balance-value.margin { color: var(--color-signal-orange); }
  .balance-value.risk { color: var(--color-signal-red); }
  .balance-value.muted { color: var(--color-text-dim); font-style: italic; font-size: 12px; }
  .kelly-preview-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    margin: 10px 0 0;
    font-size: 12px;
    font-family: 'Space Mono', ui-monospace, monospace;
    color: var(--color-text-secondary);
    border-left: 2px solid var(--color-accent-steel);
    background: color-mix(in srgb, var(--color-accent-steel) 6%, transparent);
  }
  .kelly-preview-row.negative {
    color: var(--color-signal-red);
    border-left-color: var(--color-signal-red);
    background: color-mix(in srgb, var(--color-signal-red) 8%, transparent);
  }
  .kelly-preview-row.muted {
    color: var(--color-text-dim);
    font-style: italic;
    border-left-color: var(--color-text-dim);
    background: transparent;
  }
  .kelly-preview-badge { font-size: 11px; flex-shrink: 0; }
  ${TOAST_CSS}
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
  theme?: string,
): void {
  dismiss();

  const host = document.createElement("div");
  host.id = "testudo-modal";
  if (theme && theme !== "amoled") {
    host.setAttribute("data-theme", theme);
  }
  const shadow = host.attachShadow({ mode: "open" });

  const style = document.createElement("style");
  style.textContent = MODAL_STYLES;
  shadow.appendChild(style);

  const container = document.createElement("div");
  container.setAttribute("role", "dialog");
  container.setAttribute("aria-modal", "true");
  container.setAttribute("aria-label", "Trade Confirmation");
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

  // Focus trap: Tab cycles within modal, Escape closes
  function trapFocus(e: KeyboardEvent) {
    if (e.key === "Escape") { dismiss(); onResult("dismiss", null); return; }
    if (e.key !== "Tab") return;
    const focusable = container.querySelectorAll(
      'button, input, select, textarea, [tabindex]:not([tabindex="-1"])'
    );
    if (focusable.length === 0) return;
    const first = focusable[0] as HTMLElement;
    const last = focusable[focusable.length - 1] as HTMLElement;
    if (e.shiftKey && shadow.activeElement === first) {
      e.preventDefault(); last.focus();
    } else if (!e.shiftKey && shadow.activeElement === last) {
      e.preventDefault(); first.focus();
    }
  }
  container.addEventListener("keydown", trapFocus);

  document.body.appendChild(host);
  activeHost = host;
  activeDispose = dispose;

  // Focus first focusable element
  requestAnimationFrame(() => {
    const first = container.querySelector('button, input, select, textarea, [tabindex]:not([tabindex="-1"])') as HTMLElement;
    first?.focus();
  });
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

export function getActiveHost(): HTMLElement | null {
  return activeHost;
}

// --- Toast Notifications ---

const TOAST_THEME_VARS = `
  :host {
    --color-signal-green: #22C55E;
    --color-signal-red: #EF4444;
    --color-text-secondary: #b9bec8;
    --color-bg-elevated: #1a1c23;
    --color-border: #2d303a;
  }
  :host([data-theme="light"]) {
    --color-signal-green: #05963c;
    --color-signal-red: #c81419;
    --color-text-secondary: #373e4e;
    --color-bg-elevated: #eaecf2;
    --color-border: #c3c8d4;
  }
`;

const TOAST_STYLES = `
  ${TOAST_THEME_VARS}
  .toast { font-family: 'Space Grotesk', system-ui, sans-serif; }
  ${TOAST_CSS}
`;

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
  host.id = "testudo-toast";
  const shadow = host.attachShadow({ mode: "open" });

  // Apply extension theme to toast host
  if (typeof chrome !== "undefined" && chrome.storage?.local) {
    chrome.storage.local.get("testudo-theme", (result: Record<string, string>) => {
      const theme = result["testudo-theme"];
      if (theme && theme !== "amoled") {
        host.setAttribute("data-theme", theme);
      }
    });
  }

  const style = document.createElement("style");
  style.textContent = TOAST_STYLES;
  shadow.appendChild(style);

  const toast = document.createElement("div");
  toast.className = `toast ${type}`;
  toast.setAttribute("role", "alert");
  toast.setAttribute("aria-live", "polite");
  const icon = document.createElement("span");
  icon.className = "icon";
  toast.appendChild(icon);
  const text = document.createElement("span");
  text.textContent = message;
  toast.appendChild(text);
  shadow.appendChild(toast);

  document.body.appendChild(host);
  activeToasts.push(host);

  requestAnimationFrame(() => toast.classList.add("visible"));

  const duration = type === "error" ? 5000 : 3000;
  setTimeout(() => {
    toast.classList.remove("visible");
    setTimeout(() => {
      host.remove();
      const idx = activeToasts.indexOf(host);
      if (idx !== -1) activeToasts.splice(idx, 1);
    }, 300);
  }, duration);
}

// --- Persistent Banners (for configuration errors) ---

let activeBanner: HTMLElement | null = null;

export function showBanner(message: string, action?: { label: string; url: string }): void {
  // Remove existing banner before showing new one
  if (activeBanner) {
    activeBanner.remove();
    activeBanner = null;
  }

  const host = document.createElement("div");
  host.id = "testudo-banner";
  const shadow = host.attachShadow({ mode: "open" });

  // Apply extension theme
  if (typeof chrome !== "undefined" && chrome.storage?.local) {
    chrome.storage.local.get("testudo-theme", (result: Record<string, string>) => {
      const theme = result["testudo-theme"];
      if (theme && theme !== "amoled") {
        host.setAttribute("data-theme", theme);
      }
    });
  }

  const style = document.createElement("style");
  style.textContent = TOAST_STYLES;
  shadow.appendChild(style);

  const banner = document.createElement("div");
  banner.className = "testudo-banner error";
  banner.setAttribute("role", "alert");

  const icon = document.createElement("span");
  icon.className = "icon";
  banner.appendChild(icon);

  const msg = document.createElement("span");
  msg.className = "message";
  msg.textContent = message;
  banner.appendChild(msg);

  if (action) {
    const link = document.createElement("a");
    link.className = "action";
    link.textContent = action.label;
    link.href = action.url;
    link.target = "_blank";
    link.rel = "noopener";
    banner.appendChild(link);
  }

  const dismiss = document.createElement("button");
  dismiss.className = "dismiss";
  dismiss.innerHTML = "&times;";
  dismiss.addEventListener("click", () => {
    host.remove();
    activeBanner = null;
  });
  banner.appendChild(dismiss);

  shadow.appendChild(banner);
  document.body.appendChild(host);
  activeBanner = host;

  requestAnimationFrame(() => banner.classList.add("visible"));
}

export function showOrderToast(eventType: string, message: string): void {
  const style = ORDER_EVENT_STYLES[eventType];
  showToast(message, style?.type || "success");
}
