import { createSignal, Show, onMount } from "solid-js";
import browser from "webextension-polyfill";
import { useAuth } from "../context/AuthContext";
import { WEB_APP_URL } from "../../utils";
import LoginPreview from "./LoginPreview";

type ThemeKey = "amoled" | "light";

export default function PairView(props: {
  onAuthenticated: () => void;
  onBack?: () => void;
  sessionExpired?: boolean;
}) {
  const auth = useAuth();
  const [digits, setDigits] = createSignal("");
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);
  const [showSuccess, setShowSuccess] = createSignal(false);

  const [theme, setTheme] = createSignal<ThemeKey>("amoled");
  let refs: HTMLInputElement[] = [];

  onMount(async () => {
    // requestAnimationFrame ensures DOM is painted before focus (popup context)
    requestAnimationFrame(() => refs[0]?.focus());
    const stored = await browser.storage.local.get("testudo-theme");
    const t = stored["testudo-theme"] as ThemeKey | undefined;
    if (t) setTheme(t);
  });

  function toggleTheme() {
    const next = theme() === "amoled" ? "light" : "amoled";
    setTheme(next);
    browser.storage.local.set({ "testudo-theme": next });
    localStorage.setItem("testudo-theme", next);
    if (next === "amoled") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", next);
    }
  }

  async function submitPair(code?: string) {
    const value = (code || digits()).trim();
    if (value.length !== 6) {
      setError("Code must be 6 digits");
      return;
    }

    setError("");
    setLoading(true);

    try {
      const result = await auth.pair(value);
      if (result.success) {
        setShowSuccess(true);
        setTimeout(() => props.onAuthenticated(), 800);
      } else {
        setError(result.error || "Invalid or expired code");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Connection failed");
    } finally {
      setLoading(false);
    }
  }

  function handleInput(index: number, e: InputEvent) {
    const digit = (e.target as HTMLInputElement).value.replace(/\D/g, "").slice(-1);
    const current = digits().split("");
    while (current.length <= index) current.push("");
    current[index] = digit;
    const newVal = current.join("").slice(0, 6);
    setDigits(newVal);
    (e.target as HTMLInputElement).value = digit;
    if (digit && index < 5) refs[index + 1]?.focus();
  }

  function handleKeyDown(index: number, e: KeyboardEvent) {
    if (e.key === "Backspace" && !digits()[index] && index > 0) {
      refs[index - 1]?.focus();
    }
    if (e.key === "Enter" && digits().length === 6) {
      submitPair();
    }
  }

  function handlePaste(e: ClipboardEvent) {
    e.preventDefault();
    const pasted = (e.clipboardData?.getData("text") || "").replace(/\D/g, "").slice(0, 6);
    setDigits(pasted);
    for (let i = 0; i < 6; i++) {
      if (refs[i]) refs[i].value = pasted[i] || "";
    }
    if (pasted.length === 6) {
      refs[5]?.focus();
      submitPair(pasted);
    } else {
      refs[Math.min(pasted.length, 5)]?.focus();
    }
  }

  const isDisabled = () => loading() || showSuccess();

  return (
    <div class="relative w-full h-full overflow-hidden" data-testid="pair-section">
      {/* Background: static dashboard preview */}
      <div class="absolute inset-0 login-preview-bg" aria-hidden="true">
        <LoginPreview />
      </div>

      {/* Foreground: glass pairing card */}
      <div class="absolute inset-0 flex flex-col items-center justify-center p-5">
        {/* Back button */}
        <div class="w-full max-w-[440px] flex items-center justify-between mb-2">
          <Show when={props.onBack} fallback={<div />}>
            <button
              class="btn-ghost icon-btn p-0"
              onClick={props.onBack}
              data-testid="pair-back"
              title="Back"
            >
              <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M19 12H5M12 19l-7-7 7-7" />
              </svg>
            </button>
          </Show>
          <button
            class="btn-ghost icon-btn p-0 cursor-pointer"
            onClick={toggleTheme}
            title={`Theme: ${theme() === "amoled" ? "Dark" : "Light"}`}
            aria-label="Toggle theme"
          >
            {theme() === "amoled" ? (
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="5" />
                <line x1="12" y1="1" x2="12" y2="3" />
                <line x1="12" y1="21" x2="12" y2="23" />
                <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
                <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
                <line x1="1" y1="12" x2="3" y2="12" />
                <line x1="21" y1="12" x2="23" y2="12" />
                <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
                <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
              </svg>
            ) : (
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
              </svg>
            )}
          </button>
        </div>

        <div class="login-glass-card w-full max-w-[440px] px-8 py-8">
          <Show when={showSuccess()} fallback={
            <>
              {/* Logo */}
              <div class="flex flex-col items-center">
                <h1 class="text-3xl font-sans font-bold tracking-[0.25em] text-text-primary mb-1">
                  TESTUDO
                </h1>
                <p class="text-[11px] text-text-dim font-mono tracking-[0.35em] mb-6 uppercase">
                  Trading Terminal
                </p>
              </div>

              {/* What it does — pre-pair explainer */}
              <div class="text-[11px] text-text-secondary font-mono leading-relaxed mb-6 space-y-2">
                <p>
                  Press <span class="text-text-primary">Alt+X</span> on a TradingView chart.
                  Testudo sizes the trade from your stop, asks you to confirm,
                  then routes the order to your exchange.
                </p>
                <p class="text-text-dim">
                  No funds held. Exchange keys stay on the server, never in the extension.
                </p>
              </div>

              {/* Divider */}
              <div class="w-full flex items-center gap-3 mb-8">
                <div class="flex-1 h-px bg-border-subtle" />
                <svg width="10" height="10" viewBox="0 0 10 10" class="text-text-dim">
                  <path d="M5 0L10 5L5 10L0 5Z" fill="currentColor" />
                </svg>
                <div class="flex-1 h-px bg-border-subtle" />
              </div>

              {/* Pairing Form */}
              <div class="w-full" data-testid="pair-form">
                {/* Session expired banner */}
                <Show when={props.sessionExpired}>
                  <div class="text-[11px] text-text-dim font-mono mb-4 text-center border border-border-subtle px-3 py-2">
                    Session expired — pair again to continue
                  </div>
                </Show>

                {/* Numbered instructions */}
                <ol class="text-[13px] text-text-secondary font-mono leading-relaxed mb-6 list-none space-y-2">
                  <li>
                    <span class="text-text-dim mr-1">1.</span> Visit{" "}
                    <button
                      class="btn-ghost text-text-primary hover:underline p-0 font-mono text-[13px] cursor-pointer"
                      onClick={() => window.open(`${WEB_APP_URL}/pair`, "_blank")}
                    >
                      testudo.vip/pair
                    </button>
                  </li>
                  <li>
                    <span class="text-text-dim mr-1">2.</span> Connect{" "}
                    <span class="text-text-primary">your wallet</span>
                  </li>
                  <li>
                    <span class="text-text-dim mr-1">3.</span> Paste the code below
                  </li>
                </ol>

                {/* OTP input boxes */}
                <div class="flex gap-2 justify-center mb-2" onPaste={handlePaste}>
                  {[0, 1, 2, 3, 4, 5].map((i) => (
                    <input
                      ref={(el) => (refs[i] = el)}
                      type="text"
                      inputMode="numeric"
                      maxLength={1}
                      value={digits()[i] || ""}
                      onInput={(e) => handleInput(i, e)}
                      onKeyDown={(e) => handleKeyDown(i, e)}
                      disabled={isDisabled()}
                      placeholder="_"
                      class="otp-box"
                      data-testid={`otp-${i}`}
                      autocomplete="off"
                    />
                  ))}
                </div>

                {/* Code expiry hint */}
                <p class="text-[10px] text-text-dim font-mono text-center mb-4">
                  Code expires in 5 minutes
                </p>

                {/* Error display */}
                <Show when={error()}>
                  <div
                    role="alert"
                    class="text-[12px] text-signal-red font-mono py-2.5 px-3.5 mb-4 border border-signal-red/20 bg-signal-red/5"
                    data-testid="pair-error"
                  >
                    {error()}
                  </div>
                </Show>

                {/* Submit button — UX-07: primary CTA */}
                <button
                  class={`btn-primary w-full py-3.5 text-[12px] tracking-[0.2em] font-mono mt-2 flex items-center justify-center gap-2.5 ${
                    loading() ? "opacity-70" : ""
                  }`}
                  onClick={() => submitPair()}
                  disabled={isDisabled() || digits().length !== 6}
                  data-testid="pair-btn"
                >
                  <Show when={loading()}>
                    <span
                      class="inline-block w-3.5 h-3.5 border-2 animate-spin"
                      style={{
                        "border-color": "rgba(255,255,255,0.3)",
                        "border-top-color": "white",
                        "border-radius": "50%",
                      }}
                    />
                  </Show>
                  {loading() ? "PAIRING..." : "PAIR"}
                </button>

                {/* Permissions explainer — collapsed by default */}
                <details class="mt-6 text-[10px] text-text-dim font-mono">
                  <summary class="cursor-pointer select-none hover:text-text-secondary transition-colors">
                    Why these permissions?
                  </summary>
                  <div class="mt-3 space-y-3 leading-relaxed pl-2">
                    <div>
                      <div class="text-text-secondary mb-1">Chart hosts</div>
                      <div>Reads the active chart context when you press Alt+X.</div>
                      <div class="text-[9px] text-text-dim/80 mt-1 leading-snug">
                        tradingview.com, dexscreener.com, gmx.io, hyperliquid.xyz,
                        bybit, binance, okx, bitget, gate, phemex, blofin
                      </div>
                    </div>
                    <div>
                      <div class="text-text-secondary mb-1">Testudo API</div>
                      <div>Syncs account state and streams live fill events.</div>
                      <div class="text-[9px] text-text-dim/80 mt-1 leading-snug">
                        api.testudo.vip, ws.testudo.vip
                      </div>
                    </div>
                    <div>
                      <div class="text-text-secondary mb-1">Storage</div>
                      <div>Caches your session token and paired-device state locally.</div>
                    </div>
                  </div>
                </details>
              </div>
            </>
          }>
            {/* Success checkmark */}
            <div class="flex flex-col items-center gap-2 py-8">
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none"
                   stroke="var(--color-signal-green)" stroke-width="2.5">
                <path d="M20 6L9 17l-5-5" />
              </svg>
              <span class="text-sm font-mono text-signal-green">PAIRED</span>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}
