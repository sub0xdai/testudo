import { createSignal, onMount } from "solid-js";
import browser from "webextension-polyfill";
import type { ManagementPreset } from "../../types";
import { DEFAULT_MANAGEMENT_PRESET } from "../../types";

export default function TradeManagement() {
  const [preset, setPreset] = createSignal<ManagementPreset>({ ...DEFAULT_MANAGEMENT_PRESET });

  onMount(async () => {
    const stored = await browser.storage.local.get(["managementPreset"]);
    if (stored.managementPreset) {
      setPreset(stored.managementPreset as ManagementPreset);
    }
  });

  async function save(updated: ManagementPreset) {
    setPreset(updated);
    await browser.storage.local.set({ managementPreset: updated });
  }

  function updateField<K extends keyof ManagementPreset>(key: K, value: ManagementPreset[K]) {
    save({ ...preset(), [key]: value });
  }

  function sliderStyle(value: number, min: number, max: number): string {
    const pct = ((value - min) / (max - min)) * 100;
    return `background: linear-gradient(to right, var(--color-accent-blue) 0%, var(--color-accent-blue) ${pct}%, var(--color-bg-elevated) ${pct}%)`;
  }

  /** Traffic light gradient for risk: muted green → amber → red with smooth blending */
  function riskSliderStyle(value: number): string {
    const pct = ((value - 0.1) / (10 - 0.1)) * 100;
    // Muted palette
    const green = "rgba(16, 185, 129, 0.55)";
    const amber = "rgba(245, 158, 11, 0.55)";
    const red   = "rgba(239, 68, 68, 0.55)";
    const dark  = "var(--color-bg-elevated)";
    // Zone midpoints on the track for smooth blending
    const z1 = ((2 - 0.1) / (10 - 0.1)) * 100;   // ~19% green→amber transition
    const z2 = ((5 - 0.1) / (10 - 0.1)) * 100;   // ~50% amber→red transition

    if (pct <= z1) {
      return `background: linear-gradient(to right, ${green} 0%, ${green} ${pct}%, ${dark} ${pct}%)`;
    } else if (pct <= z2) {
      return `background: linear-gradient(to right, ${green} 0%, ${amber} ${z1}%, ${amber} ${pct}%, ${dark} ${pct}%)`;
    }
    return `background: linear-gradient(to right, ${green} 0%, ${amber} ${z1}%, ${red} ${z2}%, ${red} ${pct}%, ${dark} ${pct}%)`;
  }

  function riskColor(value: number): string {
    if (value <= 2) return "#34D399";
    if (value <= 5) return "#FBBF24";
    return "#F87171";
  }

  return (
    <div class="space-y-5 px-5 py-4" data-testid="trade-management">
      {/* Risk % Slider — traffic light: green ≤2, orange ≤5, red >5 */}
      <div data-testid="risk-slider">
        <label class="flex items-center justify-between mb-3">
          <span class="text-[14px] text-zinc-200 font-sans font-semibold">Risk Per Trade</span>
          <div class="value-input-box" style={{ "border-color": riskColor(preset().risk_percent) + "40" }}>
            <input
              type="number"
              step="0.1"
              min="0.1"
              max="10"
              class="w-14 text-right text-[14px]"
              style={{ color: riskColor(preset().risk_percent) }}
              value={preset().risk_percent}
              onChange={(e) => updateField("risk_percent", parseFloat(e.target.value) || 1.0)}
              data-testid="risk-percent"
            />
            <span class="text-[13px] font-mono ml-1" style={{ color: riskColor(preset().risk_percent) }}>%</span>
          </div>
        </label>
        <input
          type="range"
          min="0.1"
          max="10"
          step="0.1"
          value={preset().risk_percent}
          onInput={(e) => updateField("risk_percent", parseFloat(e.target.value) || 1.0)}
          style={riskSliderStyle(preset().risk_percent)}
          class="w-full"
        />
        <div class="flex justify-between text-[12px] text-zinc-500 font-sans mt-1.5">
          <span>0.1%</span>
          <span class="text-emerald-400">safe</span>
          <span class="text-amber-400">caution</span>
          <span class="text-red-400">danger</span>
          <span>10%</span>
        </div>
      </div>

      <div class="divider" />

      {/* Leverage Slider — 1x to 125x (Binance Futures max) */}
      <div data-testid="leverage-slider">
        <label class="flex items-center justify-between mb-3">
          <span class="text-[14px] text-zinc-200 font-sans font-semibold">Leverage</span>
          <div class="value-input-box">
            <input
              type="number"
              step="1"
              min="1"
              max="125"
              class="w-14 text-right text-[14px]"
              value={preset().leverage}
              onChange={(e) => updateField("leverage", Math.max(1, Math.min(125, parseInt(e.target.value) || 1)))}
              data-testid="leverage-value"
            />
            <span class="text-[13px] font-mono ml-1 text-accent-blue">x</span>
          </div>
        </label>
        <input
          type="range"
          min="1"
          max="125"
          step="1"
          value={preset().leverage}
          onInput={(e) => updateField("leverage", parseInt(e.target.value) || 1)}
          style={sliderStyle(preset().leverage, 1, 125)}
          class="w-full"
        />
        <div class="flex justify-between text-[12px] text-zinc-500 font-sans mt-1.5">
          <span>1x</span>
          <span>125x</span>
        </div>
      </div>

      <div class="divider" />

      {/* Break-even % Slider */}
      <div data-testid="be-slider">
        <label class="flex items-center justify-between mb-3">
          <span class="text-[14px] text-zinc-200 font-sans font-semibold">Break-Even Trigger</span>
          <div class="value-input-box">
            <input
              type="number"
              step="5"
              min="10"
              max="100"
              class="w-14 text-right text-[14px]"
              value={preset().break_even_at}
              onChange={(e) => updateField("break_even_at", parseInt(e.target.value) || 50)}
              data-testid="break-even-at"
            />
            <span class="text-[11px] text-text-dim font-mono ml-1">%</span>
          </div>
        </label>
        <input
          type="range"
          min="10"
          max="100"
          step="5"
          value={preset().break_even_at}
          onInput={(e) => updateField("break_even_at", parseInt(e.target.value) || 50)}
          style={sliderStyle(preset().break_even_at, 10, 100)}
          class="w-full"
        />
        <div class="flex justify-between text-[12px] text-zinc-500 font-sans mt-1.5">
          <span>10%</span>
          <span>100%</span>
        </div>
      </div>

      <div class="divider" />

      {/* Trailing Stop Toggle Card */}
      <div
        class={`bg-bg-panel rounded-xl border transition-all duration-200 ${
          preset().trailing_stop.enabled ? "border-accent-blue/30 glow-blue" : "border-white/10"
        }`}
        data-testid="trailing-card"
      >
        <div class="flex items-center justify-between px-4 py-3">
          <span class="text-[14px] text-zinc-200 font-sans font-semibold">
            Trailing Stop
          </span>
          <button
            class={`px-3.5 py-1 text-[11px] font-bold tracking-wider font-sans rounded-full border ${
              preset().trailing_stop.enabled
                ? "bg-accent-blue/15 text-accent-blue border-accent-blue/30"
                : "text-text-dim border-border-subtle bg-bg-elevated"
            }`}
            onClick={() =>
              updateField("trailing_stop", {
                ...preset().trailing_stop,
                enabled: !preset().trailing_stop.enabled,
              })
            }
            data-testid="trailing-toggle"
          >
            {preset().trailing_stop.enabled ? "ON" : "OFF"}
          </button>
        </div>
        <div class={`toggle-card-body ${preset().trailing_stop.enabled ? "expanded" : ""}`}>
          <div class="px-4 pb-3">
            <div class="flex items-center gap-3">
              <input
                type="range"
                min="5"
                max="100"
                step="5"
                value={preset().trailing_stop.distance_percent}
                onInput={(e) =>
                  updateField("trailing_stop", {
                    ...preset().trailing_stop,
                    distance_percent: parseInt(e.target.value) || 25,
                  })
                }
                style={sliderStyle(preset().trailing_stop.distance_percent, 5, 100)}
                class="flex-1"
              />
              <div class="value-input-box">
                <input
                  type="number"
                  step="5"
                  min="5"
                  max="100"
                  class="w-14 text-right text-[14px]"
                  value={preset().trailing_stop.distance_percent}
                  onChange={(e) =>
                    updateField("trailing_stop", {
                      ...preset().trailing_stop,
                      distance_percent: parseInt(e.target.value) || 25,
                    })
                  }
                  data-testid="trailing-distance"
                />
                <span class="text-[11px] text-text-dim font-mono ml-1">%</span>
              </div>
            </div>
            <div class="flex justify-between text-[12px] text-zinc-500 font-sans mt-1.5">
              <span>5%</span>
              <span>100%</span>
            </div>
          </div>
        </div>
      </div>

      {/* Partial TP Toggle Card */}
      <div
        class={`bg-bg-panel rounded-xl border transition-all duration-200 ${
          preset().partial_tp.enabled ? "border-accent-blue/30 glow-blue" : "border-white/10"
        }`}
        data-testid="partial-tp-card"
      >
        <div class="flex items-center justify-between px-4 py-3">
          <span class="text-[14px] text-zinc-200 font-sans font-semibold">
            Partial Take Profit
          </span>
          <button
            class={`px-3.5 py-1 text-[11px] font-bold tracking-wider font-sans rounded-full border ${
              preset().partial_tp.enabled
                ? "bg-accent-blue/15 text-accent-blue border-accent-blue/30"
                : "text-text-dim border-border-subtle bg-bg-elevated"
            }`}
            onClick={() =>
              updateField("partial_tp", {
                ...preset().partial_tp,
                enabled: !preset().partial_tp.enabled,
              })
            }
            data-testid="partial-tp-toggle"
          >
            {preset().partial_tp.enabled ? "ON" : "OFF"}
          </button>
        </div>
        <div class={`toggle-card-body ${preset().partial_tp.enabled ? "expanded" : ""}`}>
          <div class="px-4 pb-3">
            <div class="flex items-center gap-3">
              <input
                type="range"
                min="10"
                max="100"
                step="5"
                value={preset().partial_tp.close_percent}
                onInput={(e) =>
                  updateField("partial_tp", {
                    ...preset().partial_tp,
                    close_percent: parseInt(e.target.value) || 50,
                  })
                }
                style={sliderStyle(preset().partial_tp.close_percent, 10, 100)}
                class="flex-1"
              />
              <div class="value-input-box">
                <input
                  type="number"
                  step="5"
                  min="10"
                  max="100"
                  class="w-14 text-right text-[14px]"
                  value={preset().partial_tp.close_percent}
                  onChange={(e) =>
                    updateField("partial_tp", {
                      ...preset().partial_tp,
                      close_percent: parseInt(e.target.value) || 50,
                    })
                  }
                  data-testid="partial-tp-close"
                />
                <span class="text-[11px] text-text-dim font-mono ml-1">%</span>
              </div>
            </div>
            <div class="flex justify-between text-[12px] text-zinc-500 font-sans mt-1.5">
              <span>10%</span>
              <span>100%</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
