import { createSignal, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";

export default function AuthSection(props: {
  onAuthenticated: () => void;
  onContinueWithoutAccount: () => void;
  onBack?: () => void;
}) {
  const auth = useAuth();
  const [email, setEmail] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [showPassword, setShowPassword] = createSignal(false);
  const [confirmPassword, setConfirmPassword] = createSignal("");
  const [isRegister, setIsRegister] = createSignal(false);
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);

  const inputStyle = {
    "border-radius": "6px",
    border: "1px solid rgba(255,255,255,0.08)",
    transition: "border-color 200ms ease, box-shadow 200ms ease",
  };

  async function handleSubmit() {
    setError("");

    if (isRegister() && password() !== confirmPassword()) {
      setError("Passwords do not match");
      return;
    }

    if (isRegister() && password().length < 8) {
      setError("Password must be at least 8 characters");
      return;
    }

    setLoading(true);

    let response: { success: boolean; error?: string };
    if (isRegister()) {
      response = await auth.register(email(), password());
    } else {
      response = await auth.login(email(), password());
    }

    setLoading(false);

    if (response.success) {
      setPassword("");
      setConfirmPassword("");
      setError("");
      props.onAuthenticated();
    } else {
      setError(response.error || (isRegister() ? "Registration failed" : "Login failed"));
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") handleSubmit();
  }

  async function handlePaperMode() {
    await auth.continueWithoutAccount();
    props.onContinueWithoutAccount();
  }

  return (
    <div class="flex flex-col h-full" data-testid="auth-section">
      {/* Back button (when navigating from settings) */}
      <Show when={props.onBack}>
        <div class="flex items-center gap-3 px-5 py-3.5">
          <button
            class="p-1.5 border-0 rounded-lg text-text-dim hover:text-text-primary hover:bg-bg-elevated"
            onClick={props.onBack}
            data-testid="auth-back"
            title="Back"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M19 12H5M12 19l-7-7 7-7" />
            </svg>
          </button>
        </div>
      </Show>

      <div class="flex-1 flex flex-col items-center justify-center px-8 py-6">
        {/* Logo */}
        <h1 class="text-3xl font-sans font-bold tracking-[0.25em] text-white mb-1">
          TESTUDO
        </h1>
        <p class="text-[11px] text-text-dim font-mono tracking-[0.35em] mb-8 uppercase">
          Trading Terminal
        </p>

        {/* Divider */}
        <div class="w-full flex items-center gap-3 mb-8">
          <div class="flex-1 h-px bg-border-subtle" />
          <svg width="10" height="10" viewBox="0 0 10 10" class="text-text-dim">
            <path d="M5 0L10 5L5 10L0 5Z" fill="currentColor" />
          </svg>
          <div class="flex-1 h-px bg-border-subtle" />
        </div>

        {/* Auth Form */}
        <div class="w-full" data-testid="auth-logged-out">
          <Show when={error()}>
            <div
              class="text-[12px] text-signal-red font-mono py-2.5 px-3.5 mb-4 border border-signal-red/20 bg-signal-red/5 rounded-md"
              data-testid="login-error"
            >
              {error()}
            </div>
          </Show>

          {/* Email field */}
          <div class="mb-4">
            <label class="block text-[11px] text-text-secondary font-mono font-medium mb-1 tracking-wider uppercase">
              Email
            </label>
            <input
              type="email"
              placeholder="trader@testudo.io"
              value={email()}
              onInput={(e) => setEmail(e.target.value)}
              onKeyDown={handleKeyDown}
              style={inputStyle}
              data-testid="login-email"
            />
          </div>

          {/* Password field */}
          <div class="mb-4">
            <div class="flex items-baseline justify-between mb-1">
              <label class="text-[11px] text-text-secondary font-mono font-medium tracking-wider uppercase">
                Password
              </label>
              <Show when={!isRegister()}>
                <span
                  class="text-[10px] font-mono text-text-dim tracking-wider cursor-pointer hover:text-accent-steel transition-colors"
                  onClick={() => {}}
                  tabIndex={-1}
                >
                  FORGOT?
                </span>
              </Show>
            </div>
            <div class="relative">
              <input
                type={showPassword() ? "text" : "password"}
                placeholder=""
                value={password()}
                onInput={(e) => setPassword(e.target.value)}
                onKeyDown={handleKeyDown}
                autocomplete="off"
                style={{ ...inputStyle, "padding-right": "40px" }}
                data-testid="login-password"
              />
              <button
                class="absolute right-0 top-0 h-full w-10 flex items-center justify-center border-0 bg-transparent text-text-secondary hover:text-text-primary hover:bg-transparent p-0 transition-colors"
                onClick={() => setShowPassword(!showPassword())}
                tabIndex={-1}
                type="button"
                title={showPassword() ? "Hide password" : "Show password"}
              >
                <Show
                  when={showPassword()}
                  fallback={
                    <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0" />
                      <circle cx="12" cy="12" r="3" />
                    </svg>
                  }
                >
                  <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" />
                    <path d="M14.084 14.158a3 3 0 0 1-4.242-4.242" />
                    <path d="M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" />
                    <path d="m2 2 20 20" />
                  </svg>
                </Show>
              </button>
            </div>
          </div>

          {/* Confirm password (register mode) */}
          <Show when={isRegister()}>
            <div class="mb-4">
              <label class="block text-[11px] text-text-secondary font-mono font-medium mb-1 tracking-wider uppercase">
                Confirm Password
              </label>
              <input
                type="password"
                placeholder=""
                value={confirmPassword()}
                onInput={(e) => setConfirmPassword(e.target.value)}
                onKeyDown={handleKeyDown}
                autocomplete="off"
                style={inputStyle}
                data-testid="confirm-password"
              />
            </div>
          </Show>

          {/* Submit */}
          <button
            class="w-full py-3.5 text-[12px] font-bold tracking-[0.2em] font-mono border-0 text-bg-core mt-6 flex items-center justify-center gap-2.5 rounded-md"
            style={{
              background: "var(--color-accent-steel)",
              opacity: loading() ? "0.7" : "1",
            }}
            onClick={handleSubmit}
            disabled={loading()}
            data-testid="login-btn"
          >
            <Show when={loading()}>
              <span
                class="inline-block w-3.5 h-3.5 border-2 animate-spin"
                style={{
                  "border-color": "rgba(11,14,17,0.3)",
                  "border-top-color": "var(--color-bg-core)",
                  "border-radius": "50%",
                }}
              />
            </Show>
            {loading() ? "CONNECTING..." : isRegister() ? "REGISTER" : "LOGIN"}
          </button>

          {/* Divider */}
          <div class="flex items-center gap-4 my-5">
            <div class="flex-1 h-px bg-border-subtle" />
            <span class="text-[10px] font-mono text-text-dim tracking-[0.25em] px-2">OR</span>
            <div class="flex-1 h-px bg-border-subtle" />
          </div>

          {/* Secondary actions */}
          <div class="flex flex-col gap-2">
            <button
              class="w-full py-2.5 text-[11px] tracking-[0.15em] font-mono text-text-secondary border-transparent hover:border-border-active hover:text-white rounded-md"
              onClick={() => { setIsRegister(!isRegister()); setError(""); setShowPassword(false); }}
              data-testid="toggle-register"
            >
              {isRegister() ? "ALREADY HAVE AN ACCOUNT?" : "CREATE ACCOUNT"}
            </button>

            <button
              class="w-full py-2.5 text-[10px] tracking-[0.15em] font-mono text-text-dim border-0 hover:text-text-secondary hover:bg-transparent rounded-md"
              onClick={handlePaperMode}
              data-testid="paper-mode-btn"
            >
              CONTINUE WITHOUT ACCOUNT
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
