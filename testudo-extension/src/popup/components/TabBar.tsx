export type TabId = "trade" | "positions" | "account";

interface TabBarProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  positionCount: number;
}

export default function TabBar(props: TabBarProps) {
  const tabs: { id: TabId; label: string; testId: string }[] = [
    { id: "trade", label: "Trade", testId: "tab-trade" },
    { id: "positions", label: "Positions", testId: "tab-positions" },
    { id: "account", label: "Account", testId: "tab-account" },
  ];

  return (
    <div class="flex mx-5 my-2 bg-bg-panel rounded-xl p-1" data-testid="tab-bar">
      {tabs.map((tab) => (
        <button
          class={`flex-1 py-2 text-[12px] font-sans font-semibold tracking-wide border-0 rounded-lg transition-all duration-150 ${
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
              class="ml-1.5 text-[10px] font-mono text-accent-blue bg-accent-blue/10 px-1.5 py-0.5 rounded-full"
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
