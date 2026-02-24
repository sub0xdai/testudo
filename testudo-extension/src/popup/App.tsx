import { createSignal, Match, Switch } from "solid-js";
import { AuthProvider } from "./context/AuthContext";
import AuthSection from "./components/AuthSection";
import MainView from "./components/MainView";
import SettingsView from "./components/SettingsView";

type View = "auth" | "main" | "settings";

export default function App() {
  const [view, setView] = createSignal<View>("auth");
  const [cameFromMain, setCameFromMain] = createSignal(false);

  function handleReady(authed: boolean, paperOnly: boolean) {
    if (authed || paperOnly) {
      setView("main");
    } else {
      setView("auth");
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
              onContinueWithoutAccount={() => { setCameFromMain(false); setView("main"); }}
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
