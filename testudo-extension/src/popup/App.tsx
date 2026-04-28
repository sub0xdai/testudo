import { createSignal, onMount, Show } from "solid-js";
import browser from "webextension-polyfill";
import { AuthProvider, useAuth } from "./context/AuthContext";
import MainView from "./components/MainView";
import PairView from "./components/PairView";

function AuthGate() {
  const auth = useAuth();

  return (
    <Show
      when={auth.authenticated()}
      fallback={
        <PairView
          onAuthenticated={() => auth.checkAuth()}
          sessionExpired={auth.sessionState() === "session_lost" || auth.sessionState() === "wallet_changed"}
          sessionExpiredReason={
            auth.sessionState() === "wallet_changed" ? "wallet_changed" : "session_lost"
          }
        />
      }
    >
      <MainView onLogout={() => auth.logout()} />
    </Show>
  );
}

export default function App() {
  const [ready, setReady] = createSignal(false);

  // Restore theme from browser.storage.local on every popup open
  onMount(async () => {
    const stored = await browser.storage.local.get("testudo-theme");
    const theme = stored["testudo-theme"] as string | undefined;
    if (theme && theme !== "amoled") {
      document.documentElement.setAttribute("data-theme", theme);
    }
  });

  return (
    <div class="w-full h-full bg-bg-core text-text-primary font-mono">
      <AuthProvider onReady={() => setReady(true)}>
        <Show when={ready()} fallback={
          <div class="flex items-center justify-center h-full">
            <span class="text-text-secondary text-xs">LOADING...</span>
          </div>
        }>
          <AuthGate />
        </Show>
      </AuthProvider>
    </div>
  );
}
