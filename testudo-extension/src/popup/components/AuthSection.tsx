import { createSignal, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";

export default function AuthSection() {
  const [authenticated, setAuthenticated] = createSignal(false);
  const [email, setEmail] = createSignal("");
  const [loginEmail, setLoginEmail] = createSignal("");
  const [loginPassword, setLoginPassword] = createSignal("");
  const [error, setError] = createSignal("");
  const [loading, setLoading] = createSignal(false);

  async function checkAuth() {
    const response = await browser.runtime.sendMessage({ type: "AUTH_STATUS" }) as {
      authenticated: boolean;
      email?: string;
    };
    setAuthenticated(response.authenticated);
    if (response.email) setEmail(response.email);
  }

  onMount(checkAuth);

  async function handleLogin() {
    setError("");
    setLoading(true);

    const response = await browser.runtime.sendMessage({
      type: "LOGIN",
      email: loginEmail().trim(),
      password: loginPassword(),
    }) as { success: boolean; error?: string };

    setLoading(false);

    if (response.success) {
      setLoginPassword("");
      setError("");
      checkAuth();
    } else {
      setError(response.error || "Login failed");
    }
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") handleLogin();
  }

  async function handleLogout() {
    await browser.runtime.sendMessage({ type: "LOGOUT" });
    setAuthenticated(false);
    setEmail("");
  }

  return (
    <div class="space-y-2 pb-3 mb-3 border-b border-zinc-700" data-testid="auth-section">
      <label class="block text-[11px] text-zinc-500 uppercase tracking-wide">Authentication</label>

      <Show
        when={authenticated()}
        fallback={
          <div data-testid="auth-logged-out">
            <Show when={error()}>
              <div class="text-xs text-red-500 mb-2" data-testid="login-error">{error()}</div>
            </Show>
            <input
              type="email"
              class="w-full px-2.5 py-2 mb-2 bg-[#16213e] border border-zinc-700 text-zinc-200 font-mono text-sm focus:outline-none focus:border-emerald-400"
              placeholder="email@example.com"
              value={loginEmail()}
              onInput={(e) => setLoginEmail(e.target.value)}
              data-testid="login-email"
            />
            <input
              type="password"
              class="w-full px-2.5 py-2 mb-2 bg-[#16213e] border border-zinc-700 text-zinc-200 font-mono text-sm focus:outline-none focus:border-emerald-400"
              placeholder="password"
              value={loginPassword()}
              onInput={(e) => setLoginPassword(e.target.value)}
              onKeyDown={handleKeyDown}
              data-testid="login-password"
            />
            <button
              class="w-full py-2 text-xs font-medium uppercase tracking-wide bg-emerald-400 text-[#1a1a2e] border border-emerald-400 hover:bg-emerald-300"
              onClick={handleLogin}
              disabled={loading()}
              data-testid="login-btn"
            >
              {loading() ? "Logging in..." : "Login"}
            </button>
          </div>
        }
      >
        <div data-testid="auth-logged-in">
          <div class="text-xs text-zinc-500 mb-2">
            Logged in as <span class="text-emerald-400 font-mono" data-testid="auth-email">{email()}</span>
          </div>
          <button
            class="w-full py-2 text-xs font-medium uppercase tracking-wide bg-transparent text-red-500 border border-red-500 hover:bg-red-500/10"
            onClick={handleLogout}
            data-testid="logout-btn"
          >
            Logout
          </button>
        </div>
      </Show>
    </div>
  );
}
