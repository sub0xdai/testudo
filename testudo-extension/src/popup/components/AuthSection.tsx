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
  const [confirmPassword, setConfirmPassword] = createSignal("");
  const [isRegister, setIsRegister] = createSignal(false);
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);

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
        <h1 class="text-3xl font-sans font-bold tracking-[0.25em] text-white mb-1.5">
          TESTUDO
        </h1>
        <p class="text-[13px] text-text-secondary font-sans tracking-[0.15em] mb-10">
          Trading Terminal
        </p>

        {/* Auth Form */}
        <div class="w-full space-y-5" data-testid="auth-logged-out">
          <Show when={error()}>
            <div class="text-[13px] text-signal-red font-sans py-3 px-4 bg-signal-red/10 rounded-xl" data-testid="login-error">
              {error()}
            </div>
          </Show>

          <div>
            <label class="block text-[13px] text-text-secondary font-sans font-medium mb-2">
              Email
            </label>
            <input
              type="email"
              placeholder="trader@testudo.io"
              value={email()}
              onInput={(e) => setEmail(e.target.value)}
              data-testid="login-email"
            />
          </div>

          <div>
            <label class="block text-[13px] text-text-secondary font-sans font-medium mb-2">
              Password
            </label>
            <input
              type="password"
              placeholder=""
              value={password()}
              onInput={(e) => setPassword(e.target.value)}
              onKeyDown={handleKeyDown}
              autocomplete="off"
              data-testid="login-password"
            />
          </div>

          <Show when={isRegister()}>
            <div>
              <label class="block text-[13px] text-text-secondary font-sans font-medium mb-2">
                Confirm Password
              </label>
              <input
                type="password"
                placeholder=""
                value={confirmPassword()}
                onInput={(e) => setConfirmPassword(e.target.value)}
                onKeyDown={handleKeyDown}
                autocomplete="off"
                data-testid="confirm-password"
              />
            </div>
          </Show>

          <button
            class="w-full py-3.5 text-[13px] font-bold tracking-widest font-sans rounded-xl border-0 text-white mt-2"
            style={{ background: "var(--color-accent-aqua)" }}
            onClick={handleSubmit}
            disabled={loading()}
            data-testid="login-btn"
          >
            {loading() ? "AUTHENTICATING..." : isRegister() ? "REGISTER" : "LOGIN"}
          </button>

          <button
            class="w-full py-2.5 text-[13px] tracking-wider font-sans text-text-secondary border-border-subtle hover:text-white"
            onClick={() => { setIsRegister(!isRegister()); setError(""); }}
            data-testid="toggle-register"
          >
            {isRegister() ? "already have an account? login" : "create an account"}
          </button>

          <button
            class="w-full py-2.5 text-[13px] tracking-wider font-sans text-text-dim border-0 hover:text-text-secondary hover:bg-transparent"
            onClick={handlePaperMode}
            data-testid="paper-mode-btn"
          >
            continue without account
          </button>
        </div>
      </div>
    </div>
  );
}
