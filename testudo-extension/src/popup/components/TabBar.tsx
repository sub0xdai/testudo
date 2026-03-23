export type TabId = "trade" | "positions" | "account";

interface TabBarProps {
  activeTab: TabId;
  onTabChange: (tab: TabId) => void;
  positionCount: number;
  pendingCount: number;
}

export default function TabBar(props: TabBarProps) {
  const tabs: { id: TabId; label: string; testId: string }[] = [
    { id: "trade", label: "Trade", testId: "tab-trade" },
    { id: "positions", label: "Positions", testId: "tab-positions" },
    { id: "account", label: "Account", testId: "tab-account" },
  ];

  return (
    <nav aria-label="Main navigation">
    <div role="tablist" aria-label="Main navigation" class="flex mx-5 my-2 bg-bg-panel p-1" data-testid="tab-bar">
      {tabs.map((tab) => (
        <button
          role="tab"
          aria-selected={props.activeTab === tab.id}
          aria-controls={`panel-${tab.id}`}
          id={`tab-${tab.id}`}
          class={`flex-1 py-2 text-[13px] font-sans font-semibold tracking-wide border-0 transition-colors duration-150 ${
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
              class="ml-1.5 text-[11px] font-mono text-signal-green bg-signal-green/10 px-1.5 py-0.5"
              data-testid="tab-positions-count"
            >
              {props.positionCount}
            </span>
          )}
          {tab.id === "positions" && props.pendingCount > 0 && (
            <span
              class="ml-1 text-[11px] font-mono text-signal-orange bg-signal-orange/10 px-1.5 py-0.5"
              data-testid="tab-pending-count"
            >
              {props.pendingCount}
            </span>
          )}
        </button>
      ))}
    </div>
    </nav>
  );
}
