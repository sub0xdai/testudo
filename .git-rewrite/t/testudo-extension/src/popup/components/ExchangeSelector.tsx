import { createSignal, createMemo, onMount, onCleanup, Show, For } from "solid-js";
import browser from "webextension-polyfill";
import type { ExchangeAccount } from "../../types";
import { getExchangeType, type ExchangeMode } from "../../utils";

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

export default function ExchangeSelector() {
  const [accounts, setAccounts] = createSignal<ExchangeAccount[]>([]);
  const [activeId, setActiveId] = createSignal<string | null>(null);
  const [open, setOpen] = createSignal(false);
  const [exchangeMode, setExchangeMode] = createSignal<ExchangeMode>("cex");

  const filteredAccounts = createMemo(() =>
    accounts().filter((a) => getExchangeType(a.exchange_name) === exchangeMode())
  );

  async function fetchData() {
    try {
      const [accountsRes, activeRes, modeRes] = await Promise.all([
        browser.runtime.sendMessage({ type: "LIST_EXCHANGE_ACCOUNTS" }) as Promise<{
          success?: boolean;
          data?: ExchangeAccount[];
          error?: string;
        }>,
        browser.runtime.sendMessage({ type: "GET_ACTIVE_EXCHANGE" }) as Promise<{
          exchangeId: string | null;
        }>,
        browser.runtime.sendMessage({ type: "GET_EXCHANGE_MODE" }) as Promise<{
          mode: ExchangeMode;
        }>,
      ]);
      console.log("[ExchangeSelector] fetchData:", {
        accounts: accountsRes,
        active: activeRes,
        mode: modeRes,
      });
      if (modeRes?.mode) setExchangeMode(modeRes.mode);
      if (accountsRes?.success && accountsRes.data) {
        setAccounts(accountsRes.data);
      }
      const currentActiveId = activeRes?.exchangeId || null;
      setActiveId(currentActiveId);

      // Auto-select first matching account if none active
      if (!currentActiveId && accountsRes?.data?.length) {
        const mode = modeRes?.mode || "cex";
        const matching = accountsRes.data.filter((a) => getExchangeType(a.exchange_name) === mode);
        if (matching.length) {
          const firstId = matching[0].id;
          setActiveId(firstId);
          await browser.runtime.sendMessage({
            type: "SET_ACTIVE_EXCHANGE",
            exchangeId: firstId,
          });
        }
      }
    } catch (err) {
      console.error("[ExchangeSelector] fetchData failed:", err);
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

  // Refresh when accounts or mode change
  function handleStorageChange(changes: Record<string, { oldValue?: unknown; newValue?: unknown }>) {
    if (changes.exchangeAccounts) {
      fetchData();
    }
    if (changes.exchangeMode) {
      setExchangeMode((changes.exchangeMode.newValue as ExchangeMode) || "cex");
      fetchData();
    }
    if (changes.activeCexAccountId || changes.activeDexAccountId) {
      fetchData();
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

  const activeAccount = () => filteredAccounts().find((a) => a.id === activeId());
  const activeLabel = () => {
    const acct = activeAccount();
    return acct ? capitalize(acct.exchange_name) : "No Exchange";
  };

  return (
    <Show when={filteredAccounts().length > 0}>
      <div class="relative" data-exchange-selector data-testid="exchange-selector">
        <button
          class="flex items-center justify-between gap-2 px-3 h-8 min-w-[100px] border border-border-subtle text-[11px] font-bold tracking-wider font-sans transition-colors cursor-pointer text-text-primary hover:bg-bg-elevated"
          onClick={() => setOpen(!open())}
          onKeyDown={(e) => { if (e.key === "Escape" && open()) { e.preventDefault(); setOpen(false); } }}
          title="Switch exchange account"
          aria-haspopup="listbox"
          aria-expanded={open()}
        >
          <span class="max-w-[80px] truncate">{activeLabel()}</span>
          <svg aria-hidden="true" width="8" height="8" viewBox="0 0 24 24" fill="currentColor">
            <path d="M7 10l5 5 5-5z" />
          </svg>
        </button>

        {/* Dropdown */}
        <Show when={open()}>
          <div role="listbox" aria-label="Exchange accounts" class="absolute top-full right-0 mt-1 min-w-[140px] bg-bg-elevated border border-border-subtle shadow-lg z-50 overflow-hidden">
            <For each={filteredAccounts()}>
              {(account) => (
                <button
                  role="option"
                  aria-selected={account.id === activeId()}
                  class={`w-full flex items-center gap-2 px-3 py-2.5 text-left text-[11px] font-sans border-0 transition-colors ${
                    account.id === activeId()
                      ? "bg-text-primary/10 text-text-primary font-bold"
                      : "text-text-secondary hover:bg-bg-panel hover:text-text-primary"
                  }`}
                  onClick={() => selectAccount(account.id)}
                >
                  {/* UXP-17: white indicator dot */}
                  <span
                    aria-hidden="true"
                    class={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${
                      account.id === activeId() ? "bg-text-primary" : "bg-text-dim"
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
