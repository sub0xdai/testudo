import { createSignal, Match, Switch, onMount } from "solid-js";
import browser from "webextension-polyfill";
import { AuthProvider } from "./context/AuthContext";
import PairView from "./components/PairView";
import MainView from "./components/MainView";

type View = "auth" | "main";

export default function App() {
  const [view, setViewRaw] = createSignal<View>("auth");
  const [cameFromMain, setCameFromMain] = createSignal(false);
  const [sessionExpired, setSessionExpired] = createSignal(false);

  // Restore theme from browser.storage.local on every popup open
  onMount(async () => {
    const stored = await browser.storage.local.get("testudo-theme");
    const theme = stored["testudo-theme"] as string | undefined;
    if (theme && theme !== "amoled") {
      document.documentElement.setAttribute("data-theme", theme);
    }
  });

  function setView(v: View) {
    setViewRaw(v);
    browser.storage.local.set({ popupView: v });
  }

  async function handleReady(authed: boolean) {
    if (authed) {
      setSessionExpired(false);
      setViewRaw("main");
    } else {
      // Detect session expiry: user was previously on main view but is now unauthenticated
      const stored = await browser.storage.local.get("popupView");
      if (stored.popupView === "main") {
        setSessionExpired(true);
      }
      setViewRaw("auth");
    }
  }

  function goToAuth() {
    setCameFromMain(true);
    setSessionExpired(false);
    setView("auth");
  }

  return (
    <div class="w-full h-full bg-bg-core text-text-primary font-mono">
      <AuthProvider onReady={handleReady}>
        <Switch>
          <Match when={view() === "auth"}>
            <PairView
              onAuthenticated={() => { setCameFromMain(false); setSessionExpired(false); setView("main"); }}
              onBack={cameFromMain() ? () => setView("main") : undefined}
              sessionExpired={sessionExpired()}
            />
          </Match>
          <Match when={view() === "main"}>
            <MainView onLogout={goToAuth} />
          </Match>
        </Switch>
      </AuthProvider>
    </div>
  );
}
