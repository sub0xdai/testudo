import { createSignal, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";

export default function AuthSection(props: {
  onAuthenticated: () => void;
  onContinueWithoutAccount: () => void;
}) {
  const auth = useAuth();
  const [loginEmail, setLoginEmail] = createSignal("");
  const [loginPassword, setLoginPassword] = createSignal("");
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);

  async function handleLogin() {
    setError("");
    setLoading(true);

    const response = await auth.login(loginEmail(), loginPassword());

    setLoading(false);

    if (response.success) {
      setLoginPassword("");
      setError("");
      props.onAuthenticated();
    } else {
      setError(response.error || "Login failed");
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") handleLogin();
  }

  async function handlePaperMode() {
    await auth.continueWithoutAccount();
    props.onContinueWithoutAccount();
  }

  return (
    <div class="flex flex-col items-center justify-center min-h-[300px] px-8 py-6" data-testid="auth-section">
      {/* Logo */}
      <h1 class="text-2xl font-display font-bold tracking-[0.3em] text-text-primary mb-1">
        TESTUDO
      </h1>
      <p class="text-[11px] text-text-secondary tracking-[0.2em] uppercase mb-8">
        Trading Terminal
      </p>

      {/* Login Form */}
      <div class="w-full space-y-4" data-testid="auth-logged-out">
        <Show when={error()}>
          <div class="text-xs text-signal-red font-mono py-2 border-b-2 border-signal-red" data-testid="login-error">
            {error()}
          </div>
        </Show>

        <div>
          <label class="block text-[10px] text-text-secondary uppercase tracking-widest mb-1">
            Email
          </label>
          <input
            type="email"
            placeholder="trader@testudo.io"
            value={loginEmail()}
            onInput={(e) => setLoginEmail(e.target.value)}
            data-testid="login-email"
          />
        </div>

        <div>
          <label class="block text-[10px] text-text-secondary uppercase tracking-widest mb-1">
            Password
          </label>
          <input
            type="password"
            placeholder="••••••••"
            value={loginPassword()}
            onInput={(e) => setLoginPassword(e.target.value)}
            onKeyDown={handleKeyDown}
            data-testid="login-password"
          />
        </div>

        <button
          class="w-full py-3 text-xs font-bold tracking-widest border-signal-green text-signal-green hover:bg-signal-green hover:text-bg-core mt-2"
          onClick={handleLogin}
          disabled={loading()}
          data-testid="login-btn"
        >
          {loading() ? "AUTHENTICATING..." : "LOGIN"}
        </button>

        <button
          class="w-full py-2 text-[11px] tracking-wider text-text-secondary border-0 hover:text-text-primary hover:bg-transparent"
          onClick={handlePaperMode}
          data-testid="paper-mode-btn"
        >
          continue without account
        </button>
      </div>
    </div>
  );
}
