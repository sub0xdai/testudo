import { createSignal, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";
import type { z } from "zod";
import type { UserSettingsResponseSchema } from "../../schemas";

type UserSettingsResponse = z.infer<typeof UserSettingsResponseSchema>;

type GetResult =
  | { success: true; data: UserSettingsResponse }
  | { success: false; error: string; error_code?: string };

export default function SettingsPanel() {
  const [state, setState] = createSignal<UserSettingsResponse | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function fetchSettings() {
    try {
      const res = (await browser.runtime.sendMessage({ type: "GET_USER_SETTINGS" })) as GetResult;
      if (res?.success) {
        setState(res.data);
        setError(null);
      } else {
        setError(res?.error || "Failed to load settings");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load settings");
    } finally {
      setLoading(false);
    }
  }

  async function toggle() {
    const current = state();
    if (!current || saving()) return;
    if (!current.unlocked && !current.settings.dynamic_risk_enabled) return;

    const nextEnabled = !current.settings.dynamic_risk_enabled;
    setSaving(true);
    setError(null);

    // Optimistic update
    const prev = current;
    setState({
      ...current,
      settings: { ...current.settings, dynamic_risk_enabled: nextEnabled },
    });

    try {
      const res = (await browser.runtime.sendMessage({
        type: "PATCH_USER_SETTINGS",
        dynamic_risk_enabled: nextEnabled,
      })) as GetResult;
      if (res?.success) {
        setState(res.data);
      } else {
        setState(prev);
        setError(res?.error || "Update failed");
      }
    } catch (e) {
      setState(prev);
      setError(e instanceof Error ? e.message : "Update failed");
    } finally {
      setSaving(false);
    }
  }

  onMount(fetchSettings);

  const enabled = () => state()?.settings.dynamic_risk_enabled ?? false;
  const unlocked = () => state()?.unlocked ?? false;
  const taggedCount = () => state()?.tagged_trade_count ?? 0;
  const canInteract = () => !loading() && !saving() && (unlocked() || enabled());
  const unlockedAt = () => {
    const raw = state()?.settings.dynamic_risk_unlocked_at;
    if (!raw) return null;
    const d = new Date(raw);
    return Number.isNaN(d.getTime()) ? null : d.toISOString().slice(0, 10);
  };

  return (
    <div data-testid="settings-panel">
      <span class="block text-[12px] text-text-secondary font-sans font-medium mb-2">
        Risk Engine
      </span>

      <Show when={!loading()} fallback={<p class="text-[12px] text-text-dim font-sans">Loading settings…</p>}>
        <div class="flex items-start justify-between gap-3">
          <div class="flex-1 min-w-0">
            <p class="text-[13px] text-text-primary font-sans font-medium">
              Dynamic Risk
            </p>
            <p class="text-[11px] text-text-dim font-sans mt-1 leading-snug">
              <Show
                when={unlocked() || enabled()}
                fallback={
                  <>Dynamic Risk unlocks after 30 tagged closes ({taggedCount()}/30)</>
                }
              >
                Scales sizing by your calibrated per-setup edge (Quarter-Kelly, ±2× clamp).
              </Show>
            </p>
            <Show when={(unlocked() || enabled()) && unlockedAt()}>
              <p
                class="text-[10px] text-text-dim font-sans mt-1"
                data-testid="settings-unlocked-at"
              >
                Unlocked {unlockedAt()}
              </p>
            </Show>
          </div>
          <button
            type="button"
            role="switch"
            aria-checked={enabled()}
            aria-label="Toggle Dynamic Risk"
            disabled={!canInteract()}
            data-testid="dynamic-risk-toggle"
            onClick={toggle}
            class={`relative shrink-0 w-10 h-5 border transition-colors cursor-pointer ${
              enabled()
                ? "bg-signal-green border-signal-green"
                : "bg-transparent border-border-subtle"
            } ${!canInteract() ? "opacity-40 cursor-not-allowed" : ""}`}
          >
            <span
              class={`absolute top-0.5 h-3.5 w-3.5 transition-transform ${
                enabled() ? "translate-x-5 bg-bg-core" : "translate-x-0.5 bg-text-dim"
              }`}
            />
          </button>
        </div>

        <Show when={error()}>
          <p class="mt-2 text-[11px] text-signal-red font-sans" data-testid="settings-error">
            {error()}
          </p>
        </Show>
      </Show>
    </div>
  );
}
