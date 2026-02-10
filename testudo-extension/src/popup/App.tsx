import Settings from "./components/Settings";
import TradeManagement from "./components/TradeManagement";
import AuthSection from "./components/AuthSection";
import StatusBar from "./components/StatusBar";

export default function App() {
  return (
    <div class="w-80 p-4 bg-[#1a1a2e] text-zinc-200 font-sans">
      <h1 class="text-sm font-semibold text-emerald-400 uppercase tracking-wide mb-4">
        Testudo Sniper
      </h1>
      <Settings />
      <TradeManagement />
      <AuthSection />
      <StatusBar />
    </div>
  );
}
