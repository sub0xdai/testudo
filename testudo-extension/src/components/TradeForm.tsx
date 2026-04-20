import { createSignal, createMemo, onMount, onCleanup, Show, For } from "solid-js";
import type { TradeSetup } from "../scraper";
import type { ManagementPreset, BalanceResponse } from "../types";
import type { SizingPreviewSchema } from "../schemas";
import type { z } from "zod";

type SizingPreview = z.infer<typeof SizingPreviewSchema>;

// MV3 provides Promise-based APIs natively — no polyfill needed in content scripts
const browser = (globalThis as any).browser ?? (globalThis as any).chrome;

// Module-scoped cache for setup tag suggestions. Survives modal open/close
// within a content-script lifetime so we don't round-trip for every Alt+X.
const SETUP_TAG_TTL_MS = 5 * 60 * 1000;
let setupTagCache: { tags: string[]; fetchedAt: number } | null = null;

// Module-scoped cache for QNT-01b dynamic-risk flag. Saves a round-trip per Alt+X
// for the ~100% of pre-unlock users who will never see the preview.
const DYNAMIC_RISK_TTL_MS = 5 * 60 * 1000;
let dynamicRiskCache: { enabled: boolean; fetchedAt: number } | null = null;

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

  // Setup tag (optional) + autocomplete state
  const [setupTag, setSetupTag] = createSignal(props.initialSetup?.setup_tag ?? "");
  const [suggestions, setSuggestions] = createSignal<string[]>(setupTagCache?.tags ?? []);
  const [showSuggestions, setShowSuggestions] = createSignal(false);
  const [highlightIdx, setHighlightIdx] = createSignal(0);

  // QNT-01b: Kelly sizing preview. Only rendered when dynamic_risk_enabled is on.
  const [dynamicRiskEnabled, setDynamicRiskEnabled] = createSignal(
    dynamicRiskCache && Date.now() - dynamicRiskCache.fetchedAt < DYNAMIC_RISK_TTL_MS
      ? dynamicRiskCache.enabled
      : false,
  );
  const [preview, setPreview] = createSignal<SizingPreview | null>(null);

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
    const tag = setupTag().trim();
    return {
      symbol: symbol().trim(),
      side: side(),
      entry: entry()!,
      stop: stop()!,
      target: target()!,
      timeframe: timeframe(),
      setup_tag: tag.length > 0 ? tag : null,
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

  // --- Setup tag autocomplete ---

  const filteredTags = createMemo(() => {
    const q = setupTag().trim().toLowerCase();
    const all = suggestions();
    const list = q ? all.filter((t) => t.toLowerCase().startsWith(q)) : all;
    return list.slice(0, 10);
  });

  async function loadSetupTags() {
    const now = Date.now();
    if (setupTagCache && now - setupTagCache.fetchedAt < SETUP_TAG_TTL_MS) {
      setSuggestions(setupTagCache.tags);
      return;
    }
    try {
      const res: any = await browser.runtime.sendMessage({ type: "GET_SETUP_TAGS", limit: 20 });
      if (res?.success && Array.isArray(res.data)) {
        setupTagCache = { tags: res.data, fetchedAt: now };
        setSuggestions(res.data);
      }
    } catch {
      // Silent fallback — field still usable as free-text input
    }
  }

  // --- QNT-01b: dynamic-risk detection + sizing preview ---

  async function loadDynamicRiskFlag() {
    const now = Date.now();
    if (dynamicRiskCache && now - dynamicRiskCache.fetchedAt < DYNAMIC_RISK_TTL_MS) {
      setDynamicRiskEnabled(dynamicRiskCache.enabled);
      return dynamicRiskCache.enabled;
    }
    try {
      const res: any = await browser.runtime.sendMessage({ type: "GET_USER_SETTINGS" });
      const enabled = !!(res?.success && res.data?.settings?.dynamic_risk_enabled);
      dynamicRiskCache = { enabled, fetchedAt: now };
      setDynamicRiskEnabled(enabled);
      return enabled;
    } catch {
      return false;
    }
  }

  function buildPreviewPayload() {
    const tag = setupTag().trim();
    return {
      symbol: symbol().trim(),
      side: side(),
      entry: entry()!,
      stop: stop()!,
      target: target()!,
      timeframe: timeframe(),
      setup_tag: tag.length > 0 ? tag : null,
      management: {
        risk_percent: props.management.risk_percent,
        break_even_enabled: props.management.break_even_enabled,
        break_even_at: props.management.break_even_at,
        leverage: props.management.leverage,
        trailing_stop: props.management.trailing_stop,
        partial_tp: props.management.partial_tp,
      },
    };
  }

  async function fetchPreview() {
    if (!dynamicRiskEnabled() || !isValid()) return;
    try {
      const res: any = await browser.runtime.sendMessage({
        type: "PREVIEW_TRADE_SIZING",
        payload: buildPreviewPayload(),
      });
      if (res?.success && res.data) {
        setPreview(res.data as SizingPreview);
      } else {
        setPreview(null);
      }
    } catch {
      setPreview(null);
    }
  }

  function acceptHighlightedTag() {
    const list = filteredTags();
    const idx = highlightIdx();
    if (list.length === 0 || idx < 0 || idx >= list.length) return;
    setSetupTag(list[idx]);
    setShowSuggestions(false);
    setConfirmStep(0);
  }

  function handleSetupKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      if (!showSuggestions() || filteredTags().length === 0) return;
      e.preventDefault();
      e.stopPropagation();
      setHighlightIdx((i) => Math.min(i + 1, filteredTags().length - 1));
    } else if (e.key === "ArrowUp") {
      if (!showSuggestions() || filteredTags().length === 0) return;
      e.preventDefault();
      e.stopPropagation();
      setHighlightIdx((i) => Math.max(i - 1, 0));
    } else if (e.key === "Tab" && showSuggestions() && filteredTags().length > 0) {
      e.preventDefault();
      e.stopPropagation();
      acceptHighlightedTag();
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    // Route Enter/Escape to the autocomplete dropdown when visible.
    if (showSuggestions() && filteredTags().length > 0) {
      if (e.key === "Enter") {
        e.preventDefault();
        e.stopPropagation();
        acceptHighlightedTag();
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        setShowSuggestions(false);
        return;
      }
    }

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
    // QNT-01b: probe dynamic-risk once on mount; if on, fetch preview on a valid setup.
    loadDynamicRiskFlag().then((enabled) => {
      if (enabled) fetchPreview();
    });
  });
  onCleanup(() => {
    document.removeEventListener("keydown", handleKeyDown, true);
  });

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
            "border-radius": "0",
            "text-transform": "uppercase",
            "font-family": "'Space Mono', ui-monospace, monospace",
          }}>{props.activeExchange}</span>
        </Show>
      </div>
      <div class="live-warning">Real money trade. Press Enter twice to confirm.</div>

      {/* Header: Side toggle + Symbol */}
      <div class="header">
        <div class="side-toggle" role="radiogroup" aria-label="Trade direction">
          <button
            class={`side-btn ${side() === "LONG" ? "side-btn-active-long" : ""}`}
            role="radio"
            aria-checked={side() === "LONG"}
            onClick={() => setSide("LONG")}
            data-testid="side-long"
          >LONG</button>
          <button
            class={`side-btn ${side() === "SHORT" ? "side-btn-active-short" : ""}`}
            role="radio"
            aria-checked={side() === "SHORT"}
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
            id="field-tf-symbol"
            required
            data-testid="field-symbol"
          />
          <Show when={autoFilledFields().has("symbol")}>
            <button type="button" class="auto-badge" onClick={() => { clearAutoFill("symbol"); setSymbol(""); }} title="Auto-filled — click to clear">auto</button>
          </Show>
        </div>
      </div>

      {/* Price fields */}
      <div class="rows">
        <div class="field-row">
          <label class="label" for="field-tf-entry">Entry</label>
          <div class="field-wrapper">
            <input
              class={`field-input ${!isValidEntry() && entryStr().length > 0 ? "invalid" : ""} ${autoFilledFields().has("entry") ? "auto-filled" : ""}`}
              type="text"
              inputMode="decimal"
              placeholder="0.00"
              value={entryStr()}
              onInput={(e) => handleFieldChange("entry", setEntryStr, e.currentTarget.value)}
              onFocus={(e) => autoFilledFields().has("entry") && e.currentTarget.select()}
              id="field-tf-entry"
              required
              data-testid="field-entry"
            />
            <Show when={autoFilledFields().has("entry")}>
              <button type="button" class="auto-badge" onClick={() => { clearAutoFill("entry"); setEntryStr(""); }} title="Auto-filled — click to clear">auto</button>
            </Show>
          </div>
        </div>
        <div class="field-row">
          <label class="label" for="field-tf-stop">Stop</label>
          <div class="field-wrapper">
            <input
              class={`field-input ${!isValidStop() && stopStr().length > 0 ? "invalid" : ""} ${autoFilledFields().has("stop") ? "auto-filled" : ""}`}
              type="text"
              inputMode="decimal"
              placeholder="0.00"
              value={stopStr()}
              onInput={(e) => handleFieldChange("stop", setStopStr, e.currentTarget.value)}
              onFocus={(e) => autoFilledFields().has("stop") && e.currentTarget.select()}
              id="field-tf-stop"
              required
              data-testid="field-stop"
            />
            <Show when={autoFilledFields().has("stop")}>
              <button type="button" class="auto-badge" onClick={() => { clearAutoFill("stop"); setStopStr(""); }} title="Auto-filled — click to clear">auto</button>
            </Show>
          </div>
        </div>
        <div class="field-row">
          <label class="label" for="field-tf-target">Target</label>
          <div class="field-wrapper">
            <input
              class={`field-input ${!isValidTarget() && targetStr().length > 0 ? "invalid" : ""} ${autoFilledFields().has("target") ? "auto-filled" : ""}`}
              type="text"
              inputMode="decimal"
              placeholder="0.00"
              value={targetStr()}
              onInput={(e) => handleFieldChange("target", setTargetStr, e.currentTarget.value)}
              onFocus={(e) => autoFilledFields().has("target") && e.currentTarget.select()}
              id="field-tf-target"
              required
              data-testid="field-target"
            />
            <Show when={autoFilledFields().has("target")}>
              <button type="button" class="auto-badge" onClick={() => { clearAutoFill("target"); setTargetStr(""); }} title="Auto-filled — click to clear">auto</button>
            </Show>
          </div>
        </div>
      </div>

      {/* Setup tag — the pattern/thesis for this trade. Powers Setup Breakdown analytics. */}
      <div class="rows" style={{ "margin-bottom": "10px" }}>
        <div class="field-row">
          <label class="label" for="field-tf-setup">Setup</label>
          <div class="field-wrapper">
            <input
              class="field-input"
              type="text"
              placeholder="head and shoulders, breakout, falling wedge…"
              value={setupTag()}
              maxLength={48}
              autocomplete="off"
              spellcheck={false}
              onFocus={() => { loadSetupTags(); setShowSuggestions(true); setHighlightIdx(0); }}
              onBlur={() => { setTimeout(() => setShowSuggestions(false), 150); }}
              onInput={(e) => {
                setSetupTag(e.currentTarget.value);
                setHighlightIdx(0);
                setShowSuggestions(true);
                setConfirmStep(0);
              }}
              onKeyDown={handleSetupKeyDown}
              id="field-tf-setup"
              data-testid="field-setup"
            />
            <Show when={showSuggestions() && filteredTags().length > 0}>
              <ul class="suggestions" role="listbox">
                <For each={filteredTags()}>
                  {(tag, i) => (
                    <li
                      class="suggestion-item"
                      classList={{ highlighted: i() === highlightIdx() }}
                      role="option"
                      aria-selected={i() === highlightIdx()}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        setSetupTag(tag);
                        setShowSuggestions(false);
                        setConfirmStep(0);
                      }}
                      onMouseEnter={() => setHighlightIdx(i())}
                    >
                      {tag}
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          </div>
        </div>
        <div
          style={{
            "font-size": "10px",
            "color": "var(--color-text-dim)",
            "margin-top": "4px",
            "line-height": "1.4",
            "font-family": "'Space Mono', ui-monospace, monospace",
            "letter-spacing": "0.2px",
          }}
        >
          Your thesis for this trade. Keep categories consistent — it's how you'll see which patterns actually have edge.
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

      {/* QNT-01b: Kelly sizing preview row (calibrated happy-path only in T5) */}
      <Show when={dynamicRiskEnabled() && preview()?.reasoning.kind === "calibrated"}>
        {(() => {
          const p = preview()!;
          const r = p.reasoning as Extract<SizingPreview["reasoning"], { kind: "calibrated" }>;
          return (
            <div class="kelly-preview-row" data-testid="kelly-preview-row">
              <span class="kelly-preview-badge">⚡</span>
              <span>
                Risk: {p.baseline_risk_pct.toFixed(1)}% → {p.effective_risk_pct.toFixed(1)}%
                {" "}({r.n_setup} trades, {Math.round(r.p_eff * 100)}% WR, {r.avg_r_win.toFixed(1)}R avg)
              </span>
            </div>
          );
        })()}
      </Show>

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
              border: "1px solid var(--color-border)",
              "border-radius": "0",
              background: "transparent",
              color: "var(--color-accent-steel)",
              cursor: "pointer",
              "font-family": "'Space Mono', ui-monospace, monospace",
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
              border: !isValid() ? "1px solid var(--color-border)" : (confirmStep() > 0 ? "1px solid color-mix(in srgb, var(--color-signal-green) 50%, transparent)" : "1px solid color-mix(in srgb, var(--color-signal-red) 45%, transparent)"),
              "border-radius": "0",
              background: !isValid() ? "transparent" : (confirmStep() > 0 ? "color-mix(in srgb, var(--color-signal-green) 20%, transparent)" : "color-mix(in srgb, var(--color-signal-red) 20%, transparent)"),
              color: !isValid() ? "var(--color-text-dim)" : (confirmStep() > 0 ? "var(--color-signal-green)" : "var(--color-signal-red)"),
              cursor: !isValid() ? "default" : "pointer",
              opacity: !isValid() ? "0.5" : "1",
              "font-family": "'Space Mono', ui-monospace, monospace",
            }}
          >{confirmStep() > 0 ? "Confirm Now" : "Arm Confirm"}</button>
          <span class="hint"><kbd>Esc</kbd> dismiss</span>
        </div>
      </div>
    </div>
  );
}
