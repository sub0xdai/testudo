import { createSignal, onMount, For, Show } from "solid-js";
import browser from "webextension-polyfill";
import type { ExchangeInfo, ExchangeAccount, AddExchangeAccountPayload, TestConnectionResult } from "../../types";

export default function ExchangeManager() {
  const [exchanges, setExchanges] = createSignal<ExchangeInfo[]>([]);
  const [accounts, setAccounts] = createSignal<ExchangeAccount[]>([]);
  const [activeExchangeId, setActiveExchangeId] = createSignal<string | null>(null);
  const [showForm, setShowFormRaw] = createSignal(false);

  function setShowForm(v: boolean) {
    setShowFormRaw(v);
    browser.storage.local.set({ popupShowExchangeForm: v });
  }
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal("");
  const [testResults, setTestResults] = createSignal<Record<string, TestConnectionResult>>({});
  const [testingId, setTestingId] = createSignal<string | null>(null);
  const [deletingId, setDeletingId] = createSignal<string | null>(null);

  // Form state
  const [formExchange, setFormExchange] = createSignal("");
  const [formApiKey, setFormApiKey] = createSignal("");
  const [formSecret, setFormSecret] = createSignal("");
  const [formPassphrase, setFormPassphrase] = createSignal("");
  const [formSubmitting, setFormSubmitting] = createSignal(false);

  onMount(async () => {
    const [activeRes, stored] = await Promise.all([
      browser.runtime.sendMessage({ type: "GET_ACTIVE_EXCHANGE" }) as Promise<{ exchangeId: string | null }>,
      browser.storage.local.get(["popupShowExchangeForm"]),
    ]);
    setActiveExchangeId(activeRes?.exchangeId || null);
    if (stored.popupShowExchangeForm) setShowFormRaw(true);
    await fetchData();
  });

  async function fetchData() {
    setLoading(true);
    const [exRes, accRes] = await Promise.all([
      browser.runtime.sendMessage({ type: "LIST_EXCHANGES" }) as Promise<{ success: boolean; data?: ExchangeInfo[] }>,
      browser.runtime.sendMessage({ type: "LIST_EXCHANGE_ACCOUNTS" }) as Promise<{ success: boolean; data?: ExchangeAccount[] }>,
    ]);

    if (exRes.success && exRes.data) setExchanges(exRes.data);
    if (accRes.success && accRes.data) {
      setAccounts(accRes.data);
      // Auto-select first account if none active
      if (!activeExchangeId() && accRes.data.length > 0) {
        await handleSetActive(accRes.data[0].id);
      }
    }
    setLoading(false);
  }

  async function handleSetActive(accountId: string) {
    setActiveExchangeId(accountId);
    await browser.runtime.sendMessage({ type: "SET_ACTIVE_EXCHANGE", exchangeId: accountId });
  }

  function availableExchanges(): ExchangeInfo[] {
    const connected = new Set(accounts().map(a => a.exchange_name));
    return exchanges().filter(e => !connected.has(e.id));
  }

  function clearForm() {
    setFormExchange("");
    setFormApiKey("");
    setFormSecret("");
    setFormPassphrase("");
    setError("");
  }

  async function handleAdd() {
    if (!formExchange() || !formApiKey() || !formSecret()) {
      setError("Exchange, API key, and secret are required");
      return;
    }

    setFormSubmitting(true);
    setError("");

    const payload: AddExchangeAccountPayload = {
      exchange_name: formExchange(),
      api_key: formApiKey(),
      secret: formSecret(),
    };
    if (formPassphrase()) payload.passphrase = formPassphrase();

    const response = await browser.runtime.sendMessage({
      type: "ADD_EXCHANGE_ACCOUNT",
      payload,
    }) as { success: boolean; error?: string };

    setFormSubmitting(false);

    if (response.success) {
      clearForm();
      setShowForm(false);
      await fetchData();
    } else {
      setError(response.error || "Failed to add account");
    }
  }

  async function handleTest(accountId: string) {
    setTestingId(accountId);
    const response = await browser.runtime.sendMessage({
      type: "TEST_EXCHANGE_CONNECTION",
      accountId,
    }) as { success: boolean; data?: TestConnectionResult; error?: string };

    if (response.success && response.data) {
      setTestResults({ ...testResults(), [accountId]: response.data });
    }
    setTestingId(null);
  }

  async function handleDelete(accountId: string) {
    const response = await browser.runtime.sendMessage({
      type: "DELETE_EXCHANGE_ACCOUNT",
      accountId,
    }) as { success: boolean; error?: string };

    setDeletingId(null);

    if (response.success) {
      await fetchData();
    } else {
      setError(response.error || "Failed to delete account");
    }
  }

  const needsPassphrase = () => {
    const ex = formExchange();
    return ex === "okx" || ex === "kucoin";
  };

  return (
    <div class="space-y-3">
      <div class="flex items-center justify-between">
        <label class="text-[11px] text-text-secondary font-sans font-medium">
          Exchange Accounts
        </label>
        <button
          class="text-[10px] px-2.5 py-1 font-sans font-medium text-accent-steel border-accent-steel/30 hover:bg-accent-steel/10"
          onClick={() => { setShowForm(!showForm()); if (!showForm()) clearForm(); }}
          data-testid="toggle-add-exchange"
        >
          {showForm() ? "CANCEL" : "+ ADD"}
        </button>
      </div>

      <Show when={loading()}>
        <p class="text-[11px] text-text-dim font-sans">Loading...</p>
      </Show>

      {/* Connected Accounts */}
      <For each={accounts()}>
        {(account) => {
          const result = () => testResults()[account.id];
          return (
            <div
              class={`bg-bg-panel border rounded-xl p-3 space-y-2 ${activeExchangeId() === account.id ? "border-accent-steel/40" : "border-border-subtle"}`}
              data-testid={`account-${account.exchange_name}`}
            >
              <div class="flex items-center justify-between">
                <div
                  class="flex items-center gap-2 cursor-pointer"
                  onClick={() => handleSetActive(account.id)}
                  title={activeExchangeId() === account.id ? "Active exchange" : "Set as active"}
                >
                  <span
                    class={`w-2 h-2 rounded-full ${activeExchangeId() === account.id ? "bg-accent-steel shadow-[0_0_6px_rgba(148,163,184,0.5)]" : "bg-text-dim"}`}
                  />
                  <span class="text-[13px] font-sans font-medium text-white">
                    {account.account_name || account.exchange_name}
                  </span>
                  <Show when={activeExchangeId() === account.id}>
                    <span class="text-[9px] px-1.5 py-0.5 bg-accent-steel/15 text-accent-steel rounded-full font-bold tracking-wider">ACTIVE</span>
                  </Show>
                </div>
                <div class="flex items-center gap-1.5">
                  <button
                    class="text-[10px] px-2 py-0.5 font-sans text-text-secondary border-border-subtle hover:text-accent-steel hover:border-accent-steel/30"
                    onClick={() => handleTest(account.id)}
                    disabled={testingId() === account.id}
                    data-testid={`test-${account.exchange_name}`}
                  >
                    {testingId() === account.id ? "..." : "TEST"}
                  </button>
                  <Show
                    when={deletingId() === account.id}
                    fallback={
                      <button
                        class="text-[10px] px-2 py-0.5 font-sans text-text-dim border-border-subtle hover:text-signal-red hover:border-signal-red/30"
                        onClick={() => setDeletingId(account.id)}
                        data-testid={`delete-${account.exchange_name}`}
                      >
                        DEL
                      </button>
                    }
                  >
                    <button
                      class="text-[10px] px-2 py-0.5 font-sans text-signal-red border-signal-red/30 bg-signal-red/10"
                      onClick={() => handleDelete(account.id)}
                      data-testid={`confirm-delete-${account.exchange_name}`}
                    >
                      CONFIRM
                    </button>
                    <button
                      class="text-[10px] px-2 py-0.5 font-sans text-text-dim border-border-subtle"
                      onClick={() => setDeletingId(null)}
                    >
                      NO
                    </button>
                  </Show>
                </div>
              </div>
              <Show when={result()}>
                <div class="text-[10px] font-mono text-text-secondary">
                  {result()!.status === "success" ? (
                    <span class="text-signal-green">{result()!.latency_ms}ms</span>
                  ) : (
                    <span class="text-signal-red">{result()!.message}</span>
                  )}
                </div>
              </Show>
            </div>
          );
        }}
      </For>

      <Show when={!loading() && accounts().length === 0 && !showForm()}>
        <p class="text-[11px] text-text-dim font-sans py-2">No exchange accounts connected</p>
      </Show>

      {/* Add Form */}
      <Show when={showForm()}>
        <div class="bg-bg-panel border border-border-subtle rounded-xl p-3 space-y-3" data-testid="add-exchange-form">
          <Show when={error()}>
            <div class="text-[11px] text-signal-red font-sans py-2 px-2.5 bg-signal-red/10 rounded-lg">
              {error()}
            </div>
          </Show>

          <div>
            <label class="block text-[10px] text-text-secondary font-sans font-medium mb-1">
              Exchange
            </label>
            <select
              value={formExchange()}
              onChange={(e) => setFormExchange(e.target.value)}
              data-testid="exchange-select"
            >
              <option value="">Select exchange...</option>
              <For each={availableExchanges()}>
                {(ex) => <option value={ex.id}>{ex.name}</option>}
              </For>
            </select>
          </div>

          <div>
            <label class="block text-[10px] text-text-secondary font-sans font-medium mb-1">
              API Key
            </label>
            <input
              type="password"
              value={formApiKey()}
              onInput={(e) => setFormApiKey(e.target.value)}
              autocomplete="off"
              placeholder="Enter API key"
              data-testid="api-key-input"
            />
          </div>

          <div>
            <label class="block text-[10px] text-text-secondary font-sans font-medium mb-1">
              Secret
            </label>
            <input
              type="password"
              value={formSecret()}
              onInput={(e) => setFormSecret(e.target.value)}
              autocomplete="off"
              placeholder="Enter API secret"
              data-testid="api-secret-input"
            />
          </div>

          <Show when={needsPassphrase()}>
            <div>
              <label class="block text-[10px] text-text-secondary font-sans font-medium mb-1">
                Passphrase
              </label>
              <input
                type="password"
                value={formPassphrase()}
                onInput={(e) => setFormPassphrase(e.target.value)}
                autocomplete="off"
                placeholder="Enter passphrase"
                data-testid="passphrase-input"
              />
            </div>
          </Show>

          <button
            class="w-full py-2.5 text-[11px] font-bold tracking-widest font-sans rounded-xl border-0 text-white"
            style={{ background: "var(--color-accent-steel)" }}
            onClick={handleAdd}
            disabled={formSubmitting()}
            data-testid="submit-exchange"
          >
            {formSubmitting() ? "VALIDATING..." : "ADD EXCHANGE"}
          </button>
        </div>
      </Show>
    </div>
  );
}
