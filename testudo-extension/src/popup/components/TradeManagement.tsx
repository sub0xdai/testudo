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

  return (
    <div class="space-y-4" data-testid="trade-management">
      <label class="block text-[11px] text-text-dim font-display uppercase tracking-widest">
        Trade Management
      </label>

      {/* Risk % */}
      <div class="flex items-center justify-between">
        <span class="text-xs text-text-secondary font-display uppercase tracking-wider">Risk %</span>
        <input
          type="number"
          step="0.1"
          min="0.1"
          max="10"
          class="w-20 text-right"
          value={preset().risk_percent}
          onChange={(e) => updateField("risk_percent", parseFloat(e.target.value) || 1.0)}
          data-testid="risk-percent"
        />
      </div>

      {/* Break-even at % */}
      <div class="flex items-center justify-between">
        <span class="text-xs text-text-secondary font-display uppercase tracking-wider">Break-even %</span>
        <input
          type="number"
          step="5"
          min="10"
          max="100"
          class="w-20 text-right"
          value={preset().break_even_at}
          onChange={(e) => updateField("break_even_at", parseInt(e.target.value) || 50)}
          data-testid="break-even-at"
        />
      </div>

      {/* Trailing Stop */}
      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-xs text-text-secondary font-display uppercase tracking-wider">Trailing Stop</span>
          <button
            class={`px-3 py-1 text-[10px] font-bold tracking-widest ${
              preset().trailing_stop.enabled
                ? "bg-signal-green/20 text-signal-green border-signal-green"
                : "text-text-dim border-border-grid"
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
        {preset().trailing_stop.enabled && (
          <div class="flex items-center justify-between pl-4">
            <span class="text-[11px] text-text-dim font-display uppercase tracking-wider">Distance %</span>
            <input
              type="number"
              step="5"
              min="5"
              max="100"
              class="w-20 text-right"
              value={preset().trailing_stop.distance_percent}
              onChange={(e) =>
                updateField("trailing_stop", {
                  ...preset().trailing_stop,
                  distance_percent: parseInt(e.target.value) || 25,
                })
              }
              data-testid="trailing-distance"
            />
          </div>
        )}
      </div>

      {/* Partial TP */}
      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-xs text-text-secondary font-display uppercase tracking-wider">Partial TP</span>
          <button
            class={`px-3 py-1 text-[10px] font-bold tracking-widest ${
              preset().partial_tp.enabled
                ? "bg-signal-green/20 text-signal-green border-signal-green"
                : "text-text-dim border-border-grid"
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
        {preset().partial_tp.enabled && (
          <div class="flex items-center justify-between pl-4">
            <span class="text-[11px] text-text-dim font-display uppercase tracking-wider">Close %</span>
            <input
              type="number"
              step="5"
              min="10"
              max="100"
              class="w-20 text-right"
              value={preset().partial_tp.close_percent}
              onChange={(e) =>
                updateField("partial_tp", {
                  ...preset().partial_tp,
                  close_percent: parseInt(e.target.value) || 50,
                })
              }
              data-testid="partial-tp-close"
            />
          </div>
        )}
      </div>
    </div>
  );
}
