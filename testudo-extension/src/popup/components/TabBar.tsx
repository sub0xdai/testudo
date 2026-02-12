export type TabId = "trade" | "positions" | "account";

interface TabBarProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  positionCount: number;
}

export default function TabBar(props: TabBarProps) {
  const tabs: { id: TabId; label: string; testId: string }[] = [
    { id: "trade", label: "TRADE", testId: "tab-trade" },
    { id: "positions", label: "POSITIONS", testId: "tab-positions" },
    { id: "account", label: "ACCOUNT", testId: "tab-account" },
  ];

  return (
    <div class="flex bg-bg-panel" data-testid="tab-bar">
      {tabs.map((tab) => (
        <button
          class={`flex-1 py-2 text-[11px] font-display font-bold tracking-[0.15em] border-0 border-b-2 ${
            props.activeTab === tab.id
              ? "tab-active"
              : "tab-inactive"
          }`}
          onClick={() => props.onTabChange(tab.id)}
          data-testid={tab.testId}
        >
          {tab.label}
          {tab.id === "positions" && props.positionCount > 0 && (
            <span
              class="ml-1 text-[9px] font-mono text-text-primary bg-bg-elevated px-1"
              data-testid="tab-positions-count"
            >
              {props.positionCount}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}
