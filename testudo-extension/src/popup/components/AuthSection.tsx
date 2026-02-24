import { createSignal, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";

export default function AuthSection(props: {
  onAuthenticated: () => void;
  onContinueWithoutAccount: () => void;
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
    <div class="flex flex-col items-center justify-center min-h-[300px] px-8 py-6" data-testid="auth-section">
      {/* Logo */}
      <h1 class="text-2xl font-sans font-bold tracking-[0.2em] text-white mb-1">
        TESTUDO
      </h1>
      <p class="text-[11px] text-text-secondary font-sans tracking-[0.15em] mb-8">
        Trading Terminal
      </p>

      {/* Auth Form */}
      <div class="w-full space-y-4" data-testid="auth-logged-out">
        <Show when={error()}>
          <div class="text-xs text-signal-red font-sans py-2.5 px-3 bg-signal-red/10 rounded-lg" data-testid="login-error">
            {error()}
          </div>
        </Show>

        <div>
          <label class="block text-[11px] text-text-secondary font-sans font-medium mb-1.5">
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
          <label class="block text-[11px] text-text-secondary font-sans font-medium mb-1.5">
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
            <label class="block text-[11px] text-text-secondary font-sans font-medium mb-1.5">
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
          class="w-full py-3 text-xs font-bold tracking-widest font-sans rounded-xl border-0 text-white mt-2"
          style={{ background: "linear-gradient(135deg, #3B82F6, #8B5CF6)" }}
          onClick={handleSubmit}
          disabled={loading()}
          data-testid="login-btn"
        >
          {loading() ? "AUTHENTICATING..." : isRegister() ? "REGISTER" : "LOGIN"}
        </button>

        <button
          class="w-full py-2 text-[11px] tracking-wider font-sans text-text-secondary border-0 hover:text-white hover:bg-transparent"
          onClick={() => { setIsRegister(!isRegister()); setError(""); }}
          data-testid="toggle-register"
        >
          {isRegister() ? "already have an account? login" : "create an account"}
        </button>

        <button
          class="w-full py-2.5 text-[11px] tracking-wider font-sans text-text-dim border-0 hover:text-text-secondary hover:bg-transparent"
          onClick={handlePaperMode}
          data-testid="paper-mode-btn"
        >
          continue without account
        </button>
      </div>
    </div>
  );
}
