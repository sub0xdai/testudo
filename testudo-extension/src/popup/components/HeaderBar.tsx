import ModeToggle from "./ModeToggle";
import StatusBar from "./StatusBar";

interface HeaderBarProps {
  onOpenSettings: () => void;
}

export default function HeaderBar(props: HeaderBarProps) {
  return (
    <div data-testid="header-bar" class="flex items-center justify-between px-5 py-3 border-b border-border-subtle">
      <div class="flex items-center gap-3">
        <StatusBar />
        <span class="text-[13px] font-sans font-bold tracking-[0.2em] text-text-primary">
          TESTUDO
        </span>
      </div>
      <div class="flex items-center gap-3">
        <ModeToggle compact />
        <button
          class="p-1.5 border-0 text-text-dim hover:text-text-primary hover:bg-transparent"
          onClick={props.onOpenSettings}
          data-testid="settings-btn"
          title="Settings"
        >
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </button>
      </div>
    </div>
  );
}
