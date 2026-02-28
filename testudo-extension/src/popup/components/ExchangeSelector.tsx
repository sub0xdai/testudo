import { createSignal, onMount, onCleanup, Show, For } from "solid-js";
import browser from "webextension-polyfill";
import type { ExchangeAccount } from "../../types";

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export default function ExchangeSelector() {
  const [accounts, setAccounts] = createSignal<ExchangeAccount[]>([]);
  const [activeId, setActiveId] = createSignal<string | null>(null);
  const [open, setOpen] = createSignal(false);

  async function fetchData() {
    try {
      const [accountsRes, activeRes] = await Promise.all([
        browser.runtime.sendMessage({ type: "LIST_EXCHANGE_ACCOUNTS" }) as Promise<{
          success?: boolean;
          data?: ExchangeAccount[];
        }>,
        browser.runtime.sendMessage({ type: "GET_ACTIVE_EXCHANGE" }) as Promise<{
          exchangeId: string | null;
        }>,
      ]);
      if (accountsRes?.success && accountsRes.data) {
        setAccounts(accountsRes.data);
      }
      const currentActiveId = activeRes?.exchangeId || null;
      setActiveId(currentActiveId);

      // Auto-select first account if none active (safety net for background miss)
      if (!currentActiveId && accountsRes?.data?.length) {
        const firstId = accountsRes.data[0].id;
        setActiveId(firstId);
        await browser.runtime.sendMessage({
          type: "SET_ACTIVE_EXCHANGE",
          exchangeId: firstId,
        });
      }
    } catch {
      /* non-blocking */
    }
  }

  async function selectAccount(accountId: string) {
    setActiveId(accountId);
    setOpen(false);
    await browser.runtime.sendMessage({
      type: "SET_ACTIVE_EXCHANGE",
      exchangeId: accountId,
    });
  }

  // Close dropdown on click outside
  function handleClickOutside(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest("[data-exchange-selector]")) {
      setOpen(false);
    }
  }

  // Refresh when accounts change (add/delete in settings)
  function handleStorageChange(changes: Record<string, { oldValue?: unknown; newValue?: unknown }>) {
    if (changes.activeExchangeId) {
      setActiveId((changes.activeExchangeId.newValue as string) || null);
    }
  }

  onMount(() => {
    fetchData();
    document.addEventListener("click", handleClickOutside);
    browser.storage.onChanged.addListener(handleStorageChange);
  });

  onCleanup(() => {
    document.removeEventListener("click", handleClickOutside);
    browser.storage.onChanged.removeListener(handleStorageChange);
  });

  const activeAccount = () => accounts().find((a) => a.id === activeId());
  const activeLabel = () => {
    const acct = activeAccount();
    return acct ? capitalize(acct.exchange_name) : "No Exchange";
  };

  return (
    <Show when={accounts().length > 0}>
      <div class="relative" data-exchange-selector data-testid="exchange-selector">
        {/* Trigger pill */}
        <button
          class={`flex items-center gap-1 px-2 py-1 text-[10px] font-bold tracking-wider border-0 rounded-md font-sans transition-all ${
            activeAccount()
              ? "bg-accent-green/10 text-accent-green hover:bg-accent-green/20"
              : "bg-bg-panel text-text-dim hover:text-text-secondary"
          }`}
          onClick={() => setOpen(!open())}
          title="Switch exchange account"
        >
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <path d="M8 9l4-4 4 4" />
            <path d="M16 15l-4 4-4-4" />
          </svg>
          <span class="max-w-[60px] truncate">{activeLabel()}</span>
        </button>

        {/* Dropdown */}
        <Show when={open()}>
          <div class="absolute top-full right-0 mt-1 min-w-[140px] bg-bg-elevated border border-border-subtle rounded-lg shadow-lg z-50 overflow-hidden">
            <For each={accounts()}>
              {(account) => (
                <button
                  class={`w-full flex items-center gap-2 px-3 py-2 text-left text-[11px] font-sans border-0 transition-colors ${
                    account.id === activeId()
                      ? "bg-accent-green/10 text-accent-green font-bold"
                      : "text-text-secondary hover:bg-bg-panel hover:text-text-primary"
                  }`}
                  onClick={() => selectAccount(account.id)}
                >
                  <span
                    class={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${
                      account.id === activeId() ? "bg-accent-green" : "bg-text-dim"
                    }`}
                  />
                  <span class="truncate">{capitalize(account.exchange_name)}</span>
                </button>
              )}
            </For>
          </div>
        </Show>
      </div>
    </Show>
  );
}
