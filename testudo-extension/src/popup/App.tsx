import { createSignal, Match, Switch } from "solid-js";
import { AuthProvider } from "./context/AuthContext";
import AuthSection from "./components/AuthSection";
import MainView from "./components/MainView";
import SettingsView from "./components/SettingsView";

type View = "auth" | "main" | "settings";

export default function App() {
  const [view, setView] = createSignal<View>("auth");

  function handleReady(authed: boolean, paperOnly: boolean) {
    if (authed || paperOnly) {
      setView("main");
    } else {
      setView("auth");
    }
  }

  return (
    <div class="w-[400px] min-h-[300px] bg-bg-core text-text-primary font-display">
      <AuthProvider onReady={handleReady}>
        <Switch>
          <Match when={view() === "auth"}>
            <AuthSection
              onAuthenticated={() => setView("main")}
              onContinueWithoutAccount={() => setView("main")}
            />
          </Match>
          <Match when={view() === "main"}>
            <MainView onOpenSettings={() => setView("settings")} />
          </Match>
          <Match when={view() === "settings"}>
            <SettingsView
              onBack={() => setView("main")}
              onLogout={() => setView("auth")}
            />
          </Match>
        </Switch>
      </AuthProvider>
    </div>
  );
}
