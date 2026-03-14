import { createSignal, createMemo, Show, For, onMount } from "solid-js";
import browser from "webextension-polyfill";
import type { ManagementPreset, BalanceResponse, LiveBalanceResponse } from "../../types";
import { DEFAULT_MANAGEMENT_PRESET } from "../../types";

export default function QuickTrade() {
  const [symbol, setSymbol] = createSignal("");
  const [side, setSide] = createSignal<"LONG" | "SHORT">("LONG");
  const [entryStr, setEntryStr] = createSignal("");
  const [stopStr, setStopStr] = createSignal("");
  const [targetStr, setTargetStr] = createSignal("");
  const [management, setManagement] = createSignal<ManagementPreset>({ ...DEFAULT_MANAGEMENT_PRESET });
  const [balance, setBalance] = createSignal<BalanceResponse[] | null>(null);
  const [submitting, setSubmitting] = createSignal(false);
  const [status, setStatus] = createSignal<{ type: "success" | "error"; msg: string } | null>(null);

  let enterCount = 0;

  onMount(async () => {
    try {
      const [stored, balResp] = await Promise.all([
        browser.storage.local.get(["managementPreset"]),
        browser.runtime.sendMessage({ type: "GET_BALANCE" }) as Promise<{ success: boolean; data?: LiveBalanceResponse }>,
      ]);
      if (stored.managementPreset) setManagement(stored.managementPreset as ManagementPreset);
      if (balResp?.success && balResp.data) setBalance(balResp.data.balances);
    } catch { /* non-blocking */ }
  });

  const entry = createMemo(() => { const v = parseFloat(entryStr()); return isNaN(v) ? null : v; });
  const stop = createMemo(() => { const v = parseFloat(stopStr()); return isNaN(v) ? null : v; });
  const target = createMemo(() => { const v = parseFloat(targetStr()); return isNaN(v) ? null : v; });

  const isValid = createMemo(() =>
    symbol().trim().length > 0 &&
    entry() !== null && entry()! > 0 &&
    stop() !== null && stop()! > 0 &&
    target() !== null && target()! > 0
  );

  const rr = createMemo(() => {
    const e = entry(), s = stop(), t = target();
    if (e === null || s === null || t === null) return 0;
    const risk = Math.abs(e - s);
    if (risk === 0) return 0;
    return Math.abs(t - e) / risk;
  });

  const rrClass = createMemo(() => {
    const r = rr();
    return r >= 2 ? "text-signal-green" : r >= 1 ? "text-signal-orange" : "text-signal-red";
  });

  // Balance calculations
  const usdt = () => balance()?.find((b) => b.asset === "USDT");
  const available = () => { const b = usdt(); return b ? parseFloat(b.available) : null; };
  const riskAmount = createMemo(() => {
    const avail = available();
    if (avail === null) return null;
    return (management().risk_percent / 100) * avail;
  });
  const positionSize = createMemo(() => {
    const risk = riskAmount(), e = entry(), s = stop();
    if (risk === null || e === null || s === null) return null;
    const dist = Math.abs(e - s);
    if (dist === 0) return null;
    return risk / dist;
  });
  const margin = createMemo(() => {
    const qty = positionSize(), e = entry();
    if (qty === null || e === null) return null;
    return (qty * e) / management().leverage;
  });

  async function handleConfirm() {
    if (!isValid() || submitting()) return;
    // Always require double-Enter for live trading safety
    enterCount++;
    if (enterCount < 2) return;

    setSubmitting(true);
    setStatus(null);

    try {
      const mgmt = management();
      const response = await browser.runtime.sendMessage({
        type: "EXECUTE_TRADE",
        payload: {
          symbol: symbol().trim(),
          side: side(),
          entry: entry()!,
          stop: stop()!,
          target: target()!,
          timeframe: "manual",
          management: {
            risk_percent: mgmt.risk_percent,
            break_even_enabled: mgmt.break_even_enabled,
            break_even_at: mgmt.break_even_at,
            leverage: mgmt.leverage,
            trailing_stop: mgmt.trailing_stop,
            partial_tp: mgmt.partial_tp,
          },
        },
      }) as { success: boolean; error?: string };

      if (response.success) {
        setStatus({ type: "success", msg: "Order Sent" });
        // Reset form
        setSymbol(""); setEntryStr(""); setStopStr(""); setTargetStr("");
        enterCount = 0;
      } else {
        setStatus({ type: "error", msg: response.error || "Unknown error" });
      }
    } catch (err) {
      setStatus({ type: "error", msg: err instanceof Error ? err.message : "Failed to send" });
    } finally {
      enterCount = 0;          // Always reset safety guard
      setSubmitting(false);
    }
  }

  return (
    <div class="px-5 py-4 space-y-3" data-testid="quick-trade">
      {/* Symbol */}
      <div>
        <label for="field-qt-symbol" class="block text-[11px] text-text-secondary font-sans font-medium mb-1.5">Symbol</label>
        <input
          id="field-qt-symbol"
          type="text"
          placeholder="BTCUSDT"
          value={symbol()}
          onInput={(e) => setSymbol(e.currentTarget.value)}
          class="font-mono"
          required
          data-testid="qt-symbol"
        />
      </div>

      {/* Side Toggle */}
      <div>
        <label class="block text-[11px] text-text-secondary font-sans font-medium mb-1.5" id="qt-side-label">Side</label>
        <div class="flex gap-2" role="radiogroup" aria-labelledby="qt-side-label">
          <button
            class={`flex-1 py-2 min-h-[44px] text-[12px] font-bold tracking-wider rounded-xl ${
              side() === "LONG"
                ? "bg-signal-green/15 text-signal-green border-signal-green/30"
                : "text-text-dim"
            }`}
            role="radio"
            aria-checked={side() === "LONG"}
            onClick={() => setSide("LONG")}
            data-testid="qt-side-long"
          >LONG</button>
          <button
            class={`flex-1 py-2 min-h-[44px] text-[12px] font-bold tracking-wider rounded-xl ${
              side() === "SHORT"
                ? "bg-signal-red/15 text-signal-red border-signal-red/30"
                : "text-text-dim"
            }`}
            role="radio"
            aria-checked={side() === "SHORT"}
            onClick={() => setSide("SHORT")}
            data-testid="qt-side-short"
          >SHORT</button>
        </div>
      </div>

      {/* Price Fields */}
      <div class="grid grid-cols-3 gap-2">
        <div>
          <label for="field-qt-entry" class="block text-[11px] text-text-secondary font-sans font-medium mb-1.5">Entry</label>
          <input
            id="field-qt-entry"
            type="text"
            inputMode="decimal"
            placeholder="0.00"
            value={entryStr()}
            onInput={(e) => setEntryStr(e.currentTarget.value)}
            class="font-mono text-[13px]"
            required
            data-testid="qt-entry"
          />
        </div>
        <div>
          <label for="field-qt-stop" class="block text-[11px] text-text-secondary font-sans font-medium mb-1.5">Stop</label>
          <input
            id="field-qt-stop"
            type="text"
            inputMode="decimal"
            placeholder="0.00"
            value={stopStr()}
            onInput={(e) => setStopStr(e.currentTarget.value)}
            class="font-mono text-[13px]"
            required
            data-testid="qt-stop"
          />
        </div>
        <div>
          <label for="field-qt-target" class="block text-[11px] text-text-secondary font-sans font-medium mb-1.5">Target</label>
          <input
            id="field-qt-target"
            type="text"
            inputMode="decimal"
            placeholder="0.00"
            value={targetStr()}
            onInput={(e) => setTargetStr(e.currentTarget.value)}
            class="font-mono text-[13px]"
            required
            data-testid="qt-target"
          />
        </div>
      </div>

      {/* R:R */}
      <Show when={isValid()}>
        <div class="flex justify-between items-center py-2 border-t border-border-subtle">
          <span class="text-[12px] text-text-secondary font-sans font-semibold tracking-wider uppercase">R:R</span>
          <span class={`text-[18px] font-mono font-bold ${rrClass()}`}>1 : {rr().toFixed(2)}</span>
        </div>
      </Show>

      {/* Balance Summary */}
      <Show when={isValid() && available() !== null}>
        <div class="space-y-1 py-2 border-t border-border-subtle">
          <div class="flex justify-between">
            <span class="text-[11px] text-text-secondary font-sans">Size</span>
            <span class="text-[13px] font-mono text-white font-semibold">{positionSize()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 4 })}</span>
          </div>
          <div class="flex justify-between">
            <span class="text-[11px] text-text-secondary font-sans">Margin</span>
            <span class="text-[13px] font-mono text-signal-orange">{margin()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDT</span>
          </div>
          <div class="flex justify-between">
            <span class="text-[11px] text-text-secondary font-sans">Risk</span>
            <span class="text-[13px] font-mono text-signal-red">{riskAmount()!.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USDT</span>
          </div>
        </div>
      </Show>

      {/* Status message */}
      <Show when={status()}>
        {(s) => (
          <div role="alert" class={`text-[12px] font-sans font-semibold text-center py-1 ${
            s().type === "success" ? "text-signal-green" : "text-signal-red"
          }`} data-testid="qt-status">
            {s().msg}
          </div>
        )}
      </Show>

      {/* Confirm Button */}
      <button
        class={`w-full py-3 text-[13px] font-bold tracking-widest rounded-xl transition-colors ${
          isValid() && !submitting()
            ? side() === "LONG"
              ? "bg-signal-green/15 text-signal-green border-signal-green/30 hover:bg-signal-green/25"
              : "bg-signal-red/15 text-signal-red border-signal-red/30 hover:bg-signal-red/25"
            : "opacity-30 cursor-not-allowed"
        }`}
        onClick={handleConfirm}
        disabled={!isValid() || submitting()}
        data-testid="qt-confirm"
      >
        {submitting() ? <><span class="inline-block animate-spin mr-1">&#x27F3;</span> Executing...</> : `${side()} (2x ENTER)`}
      </button>

      <div class="text-[10px] text-signal-red/70 text-center font-sans">
        LIVE MODE — Press confirm twice
      </div>
    </div>
  );
}
