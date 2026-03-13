import { createSignal, createMemo, onMount, onCleanup, Show, For } from "solid-js";
import type { TradeSetup } from "../scraper";
import type { ManagementPreset, BalanceResponse } from "../types";

export interface TradeFormProps {
  initialSetup?: TradeSetup | null;
  management: ManagementPreset;
  balance?: BalanceResponse[] | null;
  activeExchange?: string | null;
  onConfirm: (setup: TradeSetup) => void;
  onDismiss?: () => void;
}

function formatPrice(price: number): string {
  if (price >= 1000) return price.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (price >= 1) return price.toLocaleString("en-US", { minimumFractionDigits: 4, maximumFractionDigits: 4 });
  return price.toLocaleString("en-US", { minimumFractionDigits: 6, maximumFractionDigits: 8 });
}

export default function TradeForm(props: TradeFormProps) {
  const hasFullSetup = () => !!(props.initialSetup && props.initialSetup.entry > 0);

  const [symbol, setSymbol] = createSignal(props.initialSetup?.symbol ?? "");
  const [side, setSide] = createSignal<"LONG" | "SHORT">(props.initialSetup?.side ?? "LONG");
  const [entryStr, setEntryStr] = createSignal(hasFullSetup() ? String(props.initialSetup!.entry) : "");
  const [stopStr, setStopStr] = createSignal(hasFullSetup() ? String(props.initialSetup!.stop) : "");
  const [targetStr, setTargetStr] = createSignal(hasFullSetup() ? String(props.initialSetup!.target) : "");
  const timeframe = () => props.initialSetup?.timeframe ?? "manual";

  // Track which fields were auto-filled from scraper
  const initialAutoFields = (): Set<string> => {
    const fields = new Set<string>();
    if (props.initialSetup?.symbol) fields.add("symbol");
    if (hasFullSetup()) { fields.add("entry"); fields.add("stop"); fields.add("target"); }
    return fields;
  };
  const [autoFilledFields, setAutoFilledFields] = createSignal<Set<string>>(initialAutoFields());

  const [confirmStep, setConfirmStep] = createSignal(0);

  const entry = createMemo(() => { const v = parseFloat(entryStr()); return isNaN(v) ? null : v; });
  const stop = createMemo(() => { const v = parseFloat(stopStr()); return isNaN(v) ? null : v; });
  const target = createMemo(() => { const v = parseFloat(targetStr()); return isNaN(v) ? null : v; });

  const isValidEntry = createMemo(() => entry() !== null && entry()! > 0);
  const isValidStop = createMemo(() => stop() !== null && stop()! > 0);
  const isValidTarget = createMemo(() => target() !== null && target()! > 0);
  const isValidSymbol = createMemo(() => symbol().trim().length > 0);

  const isValid = createMemo(() => isValidEntry() && isValidStop() && isValidTarget() && isValidSymbol());

  const rr = createMemo(() => {
    const e = entry(), s = stop(), t = target();
    if (e === null || s === null || t === null) return 0;
    const risk = Math.abs(e - s);
    if (risk === 0) return 0;
    return Math.abs(t - e) / risk;
  });

  const rrClass = createMemo(() => {
    const r = rr();
    return r >= 2 ? "good" : r >= 1 ? "neutral" : "bad";
  });

  // Balance calculations
  const usdt = () => props.balance?.find((b) => b.asset === "USDT");
  const available = () => { const b = usdt(); return b ? parseFloat(b.available) : null; };
  const stopDistance = createMemo(() => {
    const e = entry(), s = stop();
    if (e === null || s === null) return 0;
    return Math.abs(e - s);
  });
  const riskAmount = createMemo(() => {
    const avail = available();
    if (avail === null) return null;
    return (props.management.risk_percent / 100) * avail;
  });
  const positionSize = createMemo(() => {
    const risk = riskAmount(), dist = stopDistance();
    if (risk === null || dist === 0) return null;
    return risk / dist;
  });
  const margin = createMemo(() => {
    const qty = positionSize(), e = entry();
    if (qty === null || e === null) return null;
    return (qty * e) / props.management.leverage;
  });
  const baseAsset = createMemo(() => symbol().replace(/USDT$|USD$|BUSD$/, ""));

  function clearAutoFill(field: string) {
    setAutoFilledFields((prev) => {
      const next = new Set(prev);
      next.delete(field);
      return next;
    });
  }

  function handleFieldChange(field: string, setter: (v: string) => void, value: string) {
    setter(value);
    clearAutoFill(field);
    setConfirmStep(0);
  }

  function buildSetup(): TradeSetup {
    return {
      symbol: symbol().trim(),
      side: side(),
      entry: entry()!,
      stop: stop()!,
      target: target()!,
      timeframe: timeframe(),
    };
  }

  function handleConfirm() {
    if (!isValid()) return;
    if (confirmStep() < 1) {
      setConfirmStep(1);
      return;
    }
    props.onConfirm(buildSetup());
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      props.onDismiss?.();
    } else if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      handleConfirm();
    }
  }

  // Keyboard handler
  onMount(() => {
    document.addEventListener("keydown", handleKeyDown, true);
  });
  onCleanup(() => {
    document.removeEventListener("keydown", handleKeyDown, true);
  });

  const mgmtRules = () => [
    { label: "Risk", value: `${props.management.risk_percent}%`, active: true },
    { label: "Break-even", value: `at ${props.management.break_even_at}%`, active: true },
    {
      label: "Trailing",
      value: props.management.trailing_stop.enabled ? `${props.management.trailing_stop.distance_percent}%` : "Off",
      active: props.management.trailing_stop.enabled,
    },
    {
      label: "Partial TP",
      value: props.management.partial_tp.enabled ? `${props.management.partial_tp.close_percent}%` : "Off",
      active: props.management.partial_tp.enabled,
    },
  ];

  return (
    <div class="panel live-mode">
      <div style={{ display: "flex", "align-items": "center", gap: "8px", "margin-bottom": "12px" }}>
        <span class="live-badge" style={{ "margin-bottom": "0" }}>LIVE MODE</span>
        <Show when={props.activeExchange}>
          <span style={{
            display: "inline-block",
            background: "color-mix(in srgb, var(--color-accent-steel) 12%, transparent)",
            color: "var(--color-accent-steel)",
            "font-size": "10px",
            "font-weight": "700",
            "letter-spacing": "0.5px",
            padding: "3px 10px",
            "border-radius": "20px",
            "text-transform": "uppercase",
          }}>{props.activeExchange}</span>
        </Show>
      </div>
      <div class="live-warning">Real money trade. Press Enter twice to confirm.</div>

      {/* Header: Side toggle + Symbol */}
      <div class="header">
        <div class="side-toggle">
          <button
            class={`side-btn ${side() === "LONG" ? "side-btn-active-long" : ""}`}
            onClick={() => setSide("LONG")}
            data-testid="side-long"
          >LONG</button>
          <button
            class={`side-btn ${side() === "SHORT" ? "side-btn-active-short" : ""}`}
            onClick={() => setSide("SHORT")}
            data-testid="side-short"
          >SHORT</button>
        </div>
        <div class="symbol-field">
          <input
            class={`field-input field-input-sm ${!isValidSymbol() && symbol().length > 0 ? "invalid" : ""} ${autoFilledFields().has("symbol") ? "auto-filled" : ""}`}
            type="text"
            placeholder="BTCUSDT"
            value={symbol()}
            onInput={(e) => handleFieldChange("symbol", setSymbol, e.currentTarget.value)}
            onFocus={(e) => autoFilledFields().has("symbol") && e.currentTarget.select()}
            data-testid="field-symbol"
          />
          <Show when={autoFilledFields().has("symbol")}>
            <span class="auto-badge" onClick={() => { clearAutoFill("symbol"); setSymbol(""); }} title="Auto-filled — click to clear">auto</span>
          </Show>
        </div>
      </div>

      {/* Price fields */}
      <div class="rows">
        <div class="field-row">
          <label class="label">Entry</label>
          <div class="field-wrapper">
            <input
              class={`field-input ${!isValidEntry() && entryStr().length > 0 ? "invalid" : ""} ${autoFilledFields().has("entry") ? "auto-filled" : ""}`}
              type="text"
              inputMode="decimal"
              placeholder="0.00"
              value={entryStr()}
              onInput={(e) => handleFieldChange("entry", setEntryStr, e.currentTarget.value)}
              onFocus={(e) => autoFilledFields().has("entry") && e.currentTarget.select()}
              data-testid="field-entry"
            />
            <Show when={autoFilledFields().has("entry")}>
              <span class="auto-badge" onClick={() => { clearAutoFill("entry"); setEntryStr(""); }} title="Auto-filled — click to clear">auto</span>
            </Show>
          </div>
        </div>
        <div class="field-row">
          <label class="label">Stop</label>
          <div class="field-wrapper">
            <input
              class={`field-input ${!isValidStop() && stopStr().length > 0 ? "invalid" : ""} ${autoFilledFields().has("stop") ? "auto-filled" : ""}`}
              type="text"
              inputMode="decimal"
              placeholder="0.00"
              value={stopStr()}
              onInput={(e) => handleFieldChange("stop", setStopStr, e.currentTarget.value)}
              onFocus={(e) => autoFilledFields().has("stop") && e.currentTarget.select()}
              data-testid="field-stop"
            />
            <Show when={autoFilledFields().has("stop")}>
              <span class="auto-badge" onClick={() => { clearAutoFill("stop"); setStopStr(""); }} title="Auto-filled — click to clear">auto</span>
            </Show>
          </div>
        </div>
        <div class="field-row">
          <label class="label">Target</label>
          <div class="field-wrapper">
            <input
              class={`field-input ${!isValidTarget() && targetStr().length > 0 ? "invalid" : ""} ${autoFilledFields().has("target") ? "auto-filled" : ""}`}
              type="text"
              inputMode="decimal"
              placeholder="0.00"
              value={targetStr()}
              onInput={(e) => handleFieldChange("target", setTargetStr, e.currentTarget.value)}
              onFocus={(e) => autoFilledFields().has("target") && e.currentTarget.select()}
              data-testid="field-target"
            />
            <Show when={autoFilledFields().has("target")}>
              <span class="auto-badge" onClick={() => { clearAutoFill("target"); setTargetStr(""); }} title="Auto-filled — click to clear">auto</span>
            </Show>
          </div>
        </div>
      </div>

      {/* R:R display */}
      <Show when={isValid()}>
        <hr class="divider" />
        <div class="rr-row">
          <span class="rr-label">Risk : Reward</span>
          <span class={`rr-value ${rrClass()}`}>1 : {rr().toFixed(2)}</span>
        </div>
      </Show>

      {/* Management Rules */}
      <div class="mgmt-section">
        <div class="mgmt-title">Management Rules</div>
        <For each={mgmtRules()}>
          {(rule) => (
            <div class="mgmt-rule">
              <span class={rule.active ? "on" : "off"}>{rule.label}: {rule.value}</span>
            </div>
          )}
        </For>
      </div>

      {/* Balance Summary */}
      <div class="balance-section">
        <Show when={isValid() && available() !== null} fallback={
          <Show when={!isValid()}>
            <div class="balance-row">
              <span class="balance-label">Size</span>
              <span class="balance-value muted">enter all fields</span>
            </div>
          </Show>
        }>
          <div class="balance-row">
            <span class="balance-label">Size</span>
            <span class="balance-value size">{positionSize()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 4 })} {baseAsset()}</span>
          </div>
          <div class="balance-row">
            <span class="balance-label">Leverage</span>
            <span class="balance-value leverage">{props.management.leverage}x</span>
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

      {/* Footer */}
      <div class="footer">
        <span class="hint">
          <kbd>Enter</kbd> <kbd>Enter</kbd> confirm
        </span>
        <div style={{ display: "flex", gap: "8px", "align-items": "center" }}>
          <button
            type="button"
            onClick={() => props.onDismiss?.()}
            style={{
              "font-size": "11px",
              "font-weight": "700",
              "letter-spacing": "0.6px",
              "text-transform": "uppercase",
              padding: "7px 12px",
              border: "1px solid color-mix(in srgb, var(--color-accent-steel) 30%, transparent)",
              "border-radius": "8px",
              background: "transparent",
              color: "var(--color-accent-steel)",
              cursor: "pointer",
            }}
          >Cancel</button>
          <button
            type="button"
            onClick={handleConfirm}
            disabled={!isValid()}
            style={{
              "font-size": "11px",
              "font-weight": "700",
              "letter-spacing": "0.6px",
              "text-transform": "uppercase",
              padding: "7px 12px",
              border: confirmStep() > 0 ? "1px solid color-mix(in srgb, var(--color-signal-green) 50%, transparent)" : "1px solid color-mix(in srgb, var(--color-signal-red) 45%, transparent)",
              "border-radius": "8px",
              background: !isValid() ? "rgba(63,63,70,0.45)" : (confirmStep() > 0 ? "color-mix(in srgb, var(--color-signal-green) 20%, transparent)" : "color-mix(in srgb, var(--color-signal-red) 20%, transparent)"),
              color: !isValid() ? "var(--color-text-dim)" : (confirmStep() > 0 ? "var(--color-signal-green)" : "var(--color-signal-red)"),
              cursor: !isValid() ? "not-allowed" : "pointer",
            }}
          >{confirmStep() > 0 ? "Confirm Now" : "Arm Confirm"}</button>
          <span class="hint"><kbd>Esc</kbd> dismiss</span>
        </div>
      </div>
    </div>
  );
}
