import { createSignal, Match, Switch, onMount } from "solid-js";
import browser from "webextension-polyfill";
import { AuthProvider } from "./context/AuthContext";
import AuthSection from "./components/AuthSection";
import MainView from "./components/MainView";
import SettingsView from "./components/SettingsView";

type View = "auth" | "main" | "settings";

export default function App() {
  const [view, setViewRaw] = createSignal<View>("auth");
  const [cameFromMain, setCameFromMain] = createSignal(false);

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
      const stored = await browser.storage.local.get(["popupView"]);
      const saved = stored.popupView as View | undefined;
      setViewRaw(saved === "settings" ? "settings" : "main");
    } else {
      setViewRaw("auth");
    }
  }

  function goToAuth() {
    setCameFromMain(true);
    setView("auth");
  }

  return (
    <div class="w-full h-full bg-bg-core text-text-primary font-mono">
      <AuthProvider onReady={handleReady}>
        <Switch>
          <Match when={view() === "auth"}>
            <AuthSection
              onAuthenticated={() => { setCameFromMain(false); setView("main"); }}
              onBack={cameFromMain() ? () => setView("main") : undefined}
            />
          </Match>
          <Match when={view() === "main"}>
            <MainView onOpenSettings={() => setView("settings")} />
          </Match>
          <Match when={view() === "settings"}>
            <SettingsView
              onBack={() => setView("main")}
              onLogout={goToAuth}
            />
          </Match>
        </Switch>
      </AuthProvider>
    </div>
  );
}
