import { createSignal, createMemo, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";
import type { z } from "zod";
import type { ManagementPreset } from "../../types";
import { DEFAULT_MANAGEMENT_PRESET } from "../../types";
import type { UserSettingsResponseSchema } from "../../schemas";

type UserSettingsResponse = z.infer<typeof UserSettingsResponseSchema>;

type UserSettingsResult =
  | { success: true; data: UserSettingsResponse }
  | { success: false; error: string; error_code?: string };

const DYNAMIC_RISK_HELP =
  "Scales sizing by your calibrated per-setup edge (Quarter-Kelly, ±2× clamp). Unlocks after 30 tagged closes.";

export default function TradeManagement() {
  const [preset, setPreset] = createSignal<ManagementPreset>({ ...DEFAULT_MANAGEMENT_PRESET });

  // QNT-01b (relocated 2026-04-21): Dynamic Risk toggle lives inside the Risk
  // Per Trade card — same decision context as the baseline risk slider.
  const [drState, setDrState] = createSignal<UserSettingsResponse | null>(null);
  const [drLoading, setDrLoading] = createSignal(true);
  const [drSaving, setDrSaving] = createSignal(false);
  const [drError, setDrError] = createSignal<string | null>(null);

  onMount(async () => {
    const stored = await browser.storage.local.get(["managementPreset"]);
    if (stored.managementPreset) {
      setPreset(stored.managementPreset as ManagementPreset);
    }
  });

  onMount(async () => {
    try {
      const res = (await browser.runtime.sendMessage({
        type: "GET_USER_SETTINGS",
      })) as UserSettingsResult;
      if (res?.success) {
        setDrState(res.data);
        setDrError(null);
      } else {
        setDrError(res?.error || "Failed to load settings");
      }
    } catch (e) {
      setDrError(e instanceof Error ? e.message : "Failed to load settings");
    } finally {
      setDrLoading(false);
    }
  });

  async function toggleDynamicRisk() {
    const current = drState();
    if (!current || drSaving()) return;
    if (!current.unlocked && !current.settings.dynamic_risk_enabled) return;

    const nextEnabled = !current.settings.dynamic_risk_enabled;
    setDrSaving(true);
    setDrError(null);

    const prev = current;
    setDrState({
      ...current,
      settings: { ...current.settings, dynamic_risk_enabled: nextEnabled },
    });

    try {
      const res = (await browser.runtime.sendMessage({
        type: "PATCH_USER_SETTINGS",
        dynamic_risk_enabled: nextEnabled,
      })) as UserSettingsResult;
      if (res?.success) {
        setDrState(res.data);
      } else {
        setDrState(prev);
        setDrError(res?.error || "Update failed");
      }
    } catch (e) {
      setDrState(prev);
      setDrError(e instanceof Error ? e.message : "Update failed");
    } finally {
      setDrSaving(false);
    }
  }

  const drEnabled = () => drState()?.settings.dynamic_risk_enabled ?? false;
  const drUnlocked = () => drState()?.unlocked ?? false;
  const drTaggedCount = () => drState()?.tagged_trade_count ?? 0;
  const drCanInteract = () => !drLoading() && !drSaving() && (drUnlocked() || drEnabled());

  let saveTimer: ReturnType<typeof setTimeout> | null = null;

  function updateField<K extends keyof ManagementPreset>(key: K, value: ManagementPreset[K]) {
    const updated = { ...preset(), [key]: value };
    setPreset(updated);

    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      browser.storage.local.set({ managementPreset: preset() });
    }, 200);
  }

  function sliderStyle(value: number, min: number, max: number): string {
    const pct = ((value - min) / (max - min)) * 100;
    const bg = `linear-gradient(to right, var(--color-accent-steel) 0%, var(--color-accent-steel) ${pct}%, var(--color-bg-elevated) ${pct}%)`;
    return `--slider-bg: ${bg}; background: ${bg}`;
  }

  /** Traffic light gradient for risk: muted green -> amber -> red with smooth blending */
  function riskSliderStyle(value: number): string {
    const pct = ((value - 0.1) / (10 - 0.1)) * 100;
    // Muted palette
    const green = "rgba(16, 185, 129, 0.55)";
    const amber = "rgba(245, 158, 11, 0.55)";
    const red   = "rgba(239, 68, 68, 0.55)";
    const dark  = "var(--color-bg-elevated)";
    // Zone midpoints on the track for smooth blending
    const z1 = ((2 - 0.1) / (10 - 0.1)) * 100;   // ~19% green->amber transition
    const z2 = ((5 - 0.1) / (10 - 0.1)) * 100;   // ~50% amber->red transition

    let bg: string;
    if (pct <= z1) {
      bg = `linear-gradient(to right, ${green} 0%, ${green} ${pct}%, ${dark} ${pct}%)`;
    } else if (pct <= z2) {
      bg = `linear-gradient(to right, ${green} 0%, ${amber} ${z1}%, ${amber} ${pct}%, ${dark} ${pct}%)`;
    } else {
      bg = `linear-gradient(to right, ${green} 0%, ${amber} ${z1}%, ${red} ${z2}%, ${red} ${pct}%, ${dark} ${pct}%)`;
    }
    return `--slider-bg: ${bg}; background: ${bg}`;
  }

  function riskColor(value: number): string {
    if (value <= 2) return "var(--color-signal-green)";
    if (value <= 5) return "var(--color-signal-orange)";
    return "var(--color-signal-red)";
  }

  const riskColorMemo = createMemo(() => riskColor(preset().risk_percent));

  return (
    <div class="space-y-5 px-5 py-4" data-testid="trade-management">
      {/* Risk % Slider — traffic light: green <=2, orange <=5, red >5 */}
      <div data-testid="risk-slider" class="border border-border-subtle p-4 bg-bg-surface">
        <label for="field-risk-percent" class="flex items-center justify-between mb-3">
          <span class="text-[14px] text-text-primary font-sans font-semibold">Risk Per Trade</span>
          <div class="value-input-box">
            <input
              id="field-risk-percent"
              type="number"
              step="0.1"
              min="0.1"
              max="10"
              class="w-14 text-right text-[14px]"
              style={{ color: riskColorMemo() }}
              value={preset().risk_percent}
              onChange={(e) => updateField("risk_percent", parseFloat(e.target.value) || 1.0)}
              data-testid="risk-percent"
            />
            <span class="text-[13px] font-mono ml-1" style={{ color: riskColorMemo() }}>%</span>
          </div>
        </label>
        <input
          id="field-risk-range"
          type="range"
          min="0.1"
          max="10"
          step="0.1"
          value={preset().risk_percent}
          onInput={(e) => updateField("risk_percent", parseFloat(e.target.value) || 1.0)}
          style={riskSliderStyle(preset().risk_percent)}
          class="w-full"
          aria-label="Risk per trade"
        />
        <div class="flex justify-between text-[12px] text-text-dim font-sans mt-1.5">
          <span>0.1%</span>
          <span class="text-signal-green">safe</span>
          <span class="text-signal-orange">caution</span>
          <span class="text-signal-red">danger</span>
          <span>10%</span>
        </div>

        {/* QNT-01b (relocated 2026-04-21): Dynamic Risk toggle — Kelly governor over the baseline slider above */}
        <Show when={!drLoading()}>
          <div
            class="mt-3 pt-3 border-t border-border-subtle flex items-center justify-between gap-3"
            data-testid="dynamic-risk-row"
          >
            <div class="flex items-center gap-1.5">
              <span class="text-[13px] text-text-primary font-sans font-medium">Dynamic Risk</span>
              <span
                role="img"
                aria-label="About Dynamic Risk"
                title={DYNAMIC_RISK_HELP}
                class="inline-flex items-center justify-center w-4 h-4 border border-border-subtle text-[10px] text-text-dim font-mono cursor-help select-none"
              >
                ?
              </span>
            </div>
            <div class="flex items-center gap-3">
              <Show when={!drUnlocked() && !drEnabled()}>
                <span class="text-[11px] text-text-dim font-mono" data-testid="dynamic-risk-progress">
                  {drTaggedCount()}/30
                </span>
              </Show>
              <button
                type="button"
                role="switch"
                aria-checked={drEnabled()}
                aria-label="Toggle Dynamic Risk"
                disabled={!drCanInteract()}
                data-testid="dynamic-risk-toggle"
                onClick={toggleDynamicRisk}
                class={`relative shrink-0 w-10 h-5 border transition-colors cursor-pointer ${
                  drEnabled()
                    ? "bg-signal-green border-signal-green"
                    : "bg-transparent border-border-subtle"
                } ${!drCanInteract() ? "opacity-40 cursor-not-allowed" : ""}`}
              >
                <span
                  class={`absolute top-0.5 h-3.5 w-3.5 transition-transform ${
                    drEnabled() ? "translate-x-5 bg-bg-core" : "translate-x-0.5 bg-text-dim"
                  }`}
                />
              </button>
            </div>
          </div>
          <Show when={drError()}>
            <p class="mt-2 text-[11px] text-signal-red font-sans" data-testid="dynamic-risk-error">
              {drError()}
            </p>
          </Show>
        </Show>
      </div>

      <div class="divider" />

      {/* Leverage Slider — 1x to 125x (Binance Futures max) */}
      <div data-testid="leverage-slider" class="border border-border-subtle p-4 bg-bg-surface">
        <label for="field-leverage" class="flex items-center justify-between mb-3">
          <span class="text-[14px] text-text-primary font-sans font-semibold">Leverage</span>
          <div class="value-input-box">
            <input
              id="field-leverage"
              type="number"
              step="1"
              min="1"
              max="125"
              class="w-14 text-right text-[14px]"
              value={preset().leverage}
              onChange={(e) => updateField("leverage", Math.max(1, Math.min(125, parseInt(e.target.value) || 1)))}
              data-testid="leverage-value"
            />
            <span class="text-[13px] font-mono ml-1 text-accent-steel">x</span>
          </div>
        </label>
        <input
          id="field-leverage-range"
          type="range"
          min="1"
          max="125"
          step="1"
          value={preset().leverage}
          onInput={(e) => updateField("leverage", parseInt(e.target.value) || 1)}
          style={sliderStyle(preset().leverage, 1, 125)}
          class="w-full"
          aria-label="Leverage"
        />
        <div class="flex justify-between text-[12px] text-text-dim font-sans mt-1.5">
          <span>1x</span>
          <span>125x</span>
        </div>
      </div>

      <div class="divider" />

      {/* Break-even — Coming Soon */}
      <div class="bg-bg-panel border border-text-primary/5 opacity-40 pointer-events-none" data-testid="be-slider">
        <div class="flex items-center justify-between px-4 py-3">
          <span class="text-[14px] text-text-primary font-sans font-semibold">Break-Even Trigger</span>
          <span class="px-3.5 py-1.5 text-[10px] font-bold tracking-widest font-mono border border-border-subtle bg-bg-elevated text-text-dim">COMING SOON</span>
        </div>
      </div>

      {/* Trailing Stop — Coming Soon */}
      <div class="bg-bg-panel border border-text-primary/5 opacity-40 pointer-events-none" data-testid="trailing-card">
        <div class="flex items-center justify-between px-4 py-3">
          <span class="text-[14px] text-text-primary font-sans font-semibold">Trailing Stop</span>
          <span class="px-3.5 py-1.5 text-[10px] font-bold tracking-widest font-mono border border-border-subtle bg-bg-elevated text-text-dim">COMING SOON</span>
        </div>
      </div>

      {/* Partial TP — Coming Soon */}
      <div class="bg-bg-panel border border-text-primary/5 opacity-40 pointer-events-none" data-testid="partial-tp-card">
        <div class="flex items-center justify-between px-4 py-3">
          <span class="text-[14px] text-text-primary font-sans font-semibold">Partial Take Profit</span>
          <span class="px-3.5 py-1.5 text-[10px] font-bold tracking-widest font-mono border border-border-subtle bg-bg-elevated text-text-dim">COMING SOON</span>
        </div>
      </div>
    </div>
  );
}
