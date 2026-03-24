import { createSignal, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";
import { WEB_APP_URL } from "../../utils";
import LoginPreview from "./LoginPreview";

export default function PairView(props: {
  onAuthenticated: () => void;
  onBack?: () => void;
}) {
  const auth = useAuth();
  const [code, setCode] = createSignal("");
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);

  async function handlePair() {
    const value = code().trim();
    if (value.length !== 6) {
      setError("Code must be 6 digits");
      return;
    }

    setError("");
    setLoading(true);

    try {
      const result = await auth.pair(value);
      if (result.success) {
        setCode("");
        props.onAuthenticated();
      } else {
        setError(result.error || "Invalid or expired code");
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Connection failed");
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") handlePair();
  }

  function handleInput(e: InputEvent & { currentTarget: HTMLInputElement }) {
    // Allow only digits
    const filtered = e.currentTarget.value.replace(/\D/g, "").slice(0, 6);
    setCode(filtered);
    e.currentTarget.value = filtered;
  }

  return (
    <div class="relative w-full h-full overflow-hidden" data-testid="pair-section">
      {/* Background: static dashboard preview */}
      <div class="absolute inset-0 login-preview-bg" aria-hidden="true">
        <LoginPreview />
      </div>

      {/* Foreground: glass pairing card */}
      <div class="absolute inset-0 flex flex-col items-center justify-center p-5">
        {/* Back button (when navigating from settings) */}
        <Show when={props.onBack}>
          <div class="w-full max-w-[440px] flex items-center gap-3 mb-2">
            <button
              class="icon-btn border-0 text-text-dim hover:text-text-primary hover:bg-text-primary/5"
              onClick={props.onBack}
              data-testid="pair-back"
              title="Back"
            >
              <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M19 12H5M12 19l-7-7 7-7" />
              </svg>
            </button>
          </div>
        </Show>

        <div class="login-glass-card w-full max-w-[440px] px-8 py-8">
          {/* Logo */}
          <div class="flex flex-col items-center">
            <h1 class="text-3xl font-sans font-bold tracking-[0.25em] text-text-primary mb-1">
              TESTUDO
            </h1>
            <p class="text-[11px] text-text-dim font-mono tracking-[0.35em] mb-8 uppercase">
              Trading Terminal
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
            {/* Instructions */}
            <p class="text-[11px] text-text-secondary font-mono leading-relaxed mb-6 text-center">
              Log in at{" "}
              <button
                class="text-text-primary hover:underline border-0 bg-transparent p-0 font-mono text-[11px] cursor-pointer"
                onClick={() => window.open(WEB_APP_URL, "_blank")}
              >
                testudo.xyz
              </button>
              , then click <span class="text-text-primary">Pair Extension</span> in your account settings to get a 6-digit code.
            </p>

            <Show when={error()}>
              <div
                role="alert"
                class="text-[12px] text-signal-red font-mono py-2.5 px-3.5 mb-4 border border-signal-red/20 bg-signal-red/5"
                data-testid="pair-error"
              >
                {error()}
              </div>
            </Show>

            {/* Code field */}
            <div class="mb-4">
              <label for="field-code" class="block text-[11px] text-text-secondary font-mono font-medium mb-1 tracking-wider uppercase">
                Pairing Code
              </label>
              <input
                id="field-code"
                type="text"
                inputMode="numeric"
                pattern="[0-9]*"
                placeholder="000000"
                value={code()}
                onInput={handleInput}
                onKeyDown={handleKeyDown}
                maxLength={6}
                autocomplete="off"
                style={{
                  border: "1px solid rgba(255,255,255,0.08)",
                  transition: "border-color 200ms ease, box-shadow 200ms ease",
                  "letter-spacing": "0.5em",
                  "text-align": "center",
                  "font-size": "20px",
                }}
                data-testid="pair-code"
              />
            </div>

            {/* Submit */}
            <button
              class={`w-full py-3.5 text-[12px] font-bold tracking-[0.2em] font-mono mt-6 flex items-center justify-center gap-2.5 border border-text-primary text-text-primary hover:bg-text-primary hover:text-bg-core transition-colors ${
                loading() ? "opacity-70" : ""
              }`}
              onClick={handlePair}
              disabled={loading() || code().length !== 6}
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
          </div>
        </div>
      </div>
    </div>
  );
}
