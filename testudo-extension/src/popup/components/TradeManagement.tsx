import { createSignal, onMount, Show } from "solid-js";
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
    return `background: linear-gradient(to right, var(--color-signal-green) ${pct}%, var(--color-bg-elevated) ${pct}%)`;
  }

  return (
    <div class="space-y-5 px-5 py-4" data-testid="trade-management">
      {/* Risk % Slider */}
      <div data-testid="risk-slider">
        <label class="block text-[11px] text-text-secondary uppercase tracking-[0.15em] font-bold mb-3">
          Risk Per Trade
        </label>
        <div class="flex items-center gap-3">
          <input
            type="range"
            min="0.1"
            max="10"
            step="0.1"
            value={preset().risk_percent}
            onInput={(e) => updateField("risk_percent", parseFloat(e.target.value) || 1.0)}
            style={sliderStyle(preset().risk_percent, 0.1, 10)}
            class="flex-1"
          />
          <div class="value-input-box">
            <input
              type="number"
              step="0.1"
              min="0.1"
              max="10"
              class="w-12 text-right text-sm"
              value={preset().risk_percent}
              onChange={(e) => updateField("risk_percent", parseFloat(e.target.value) || 1.0)}
              data-testid="risk-percent"
            />
            <span class="text-[11px] text-text-dim ml-1">%</span>
          </div>
        </div>
        <div class="flex justify-between text-[9px] text-text-dim font-mono mt-1.5">
          <span>0.1</span>
          <span>10.0</span>
        </div>
      </div>

      <div class="divider" />

      {/* Break-even % Slider */}
      <div data-testid="be-slider">
        <label class="block text-[11px] text-text-secondary uppercase tracking-[0.15em] font-bold mb-3">
          Break-Even Trigger
        </label>
        <div class="flex items-center gap-3">
          <input
            type="range"
            min="10"
            max="100"
            step="5"
            value={preset().break_even_at}
            onInput={(e) => updateField("break_even_at", parseInt(e.target.value) || 50)}
            style={sliderStyle(preset().break_even_at, 10, 100)}
            class="flex-1"
          />
          <div class="value-input-box">
            <input
              type="number"
              step="5"
              min="10"
              max="100"
              class="w-12 text-right text-sm"
              value={preset().break_even_at}
              onChange={(e) => updateField("break_even_at", parseInt(e.target.value) || 50)}
              data-testid="break-even-at"
            />
            <span class="text-[11px] text-text-dim ml-1">%</span>
          </div>
        </div>
        <div class="flex justify-between text-[9px] text-text-dim font-mono mt-1.5">
          <span>10</span>
          <span>100</span>
        </div>
      </div>

      <div class="divider" />

      {/* Trailing Stop Toggle Card */}
      <div
        class={`border bg-bg-panel card-depth ${
          preset().trailing_stop.enabled ? "border-signal-green glow-green" : "border-border-grid"
        }`}
        data-testid="trailing-card"
      >
        <div class="flex items-center justify-between px-4 py-3">
          <span class="text-[11px] text-text-secondary uppercase tracking-[0.15em] font-bold">
            Trailing Stop
          </span>
          <button
            class={`px-3 py-1 text-[10px] font-bold tracking-widest ${
              preset().trailing_stop.enabled
                ? "bg-signal-green/20 text-signal-green border-signal-green"
                : "text-text-dim border-border-grid bg-bg-elevated"
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
                  class="w-12 text-right text-sm"
                  value={preset().trailing_stop.distance_percent}
                  onChange={(e) =>
                    updateField("trailing_stop", {
                      ...preset().trailing_stop,
                      distance_percent: parseInt(e.target.value) || 25,
                    })
                  }
                  data-testid="trailing-distance"
                />
                <span class="text-[11px] text-text-dim ml-1">%</span>
              </div>
            </div>
            <div class="flex justify-between text-[9px] text-text-dim font-mono mt-1.5">
              <span>5</span>
              <span>100</span>
            </div>
          </div>
        </div>
      </div>

      {/* Partial TP Toggle Card */}
      <div
        class={`border bg-bg-panel card-depth ${
          preset().partial_tp.enabled ? "border-signal-green glow-green" : "border-border-grid"
        }`}
        data-testid="partial-tp-card"
      >
        <div class="flex items-center justify-between px-4 py-3">
          <span class="text-[11px] text-text-secondary uppercase tracking-[0.15em] font-bold">
            Partial Take Profit
          </span>
          <button
            class={`px-3 py-1 text-[10px] font-bold tracking-widest ${
              preset().partial_tp.enabled
                ? "bg-signal-green/20 text-signal-green border-signal-green"
                : "text-text-dim border-border-grid bg-bg-elevated"
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
                  class="w-12 text-right text-sm"
                  value={preset().partial_tp.close_percent}
                  onChange={(e) =>
                    updateField("partial_tp", {
                      ...preset().partial_tp,
                      close_percent: parseInt(e.target.value) || 50,
                    })
                  }
                  data-testid="partial-tp-close"
                />
                <span class="text-[11px] text-text-dim ml-1">%</span>
              </div>
            </div>
            <div class="flex justify-between text-[9px] text-text-dim font-mono mt-1.5">
              <span>10</span>
              <span>100</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
