import { createSignal, createEffect, onCleanup, Show } from 'solid-js'
import type { ExchangeAccount, TestConnectionResult, ExchangeBalanceResponse } from '../../api/client'

interface KebabMenuProps {
  onTest: () => void
  onDelete: () => void
  onRevoke: () => void
  showRevoke: boolean
  isTesting: boolean
}

function KebabMenu(props: KebabMenuProps) {
  const [open, setOpen] = createSignal(false)
  const [confirmAction, setConfirmAction] = createSignal<'delete' | 'revoke' | null>(null)
  let ref: HTMLDivElement | undefined

  createEffect(() => {
    if (!open()) return
    const handler = (e: MouseEvent) => {
      if (ref && !ref.contains(e.target as Node)) {
        setOpen(false)
        setConfirmAction(null)
      }
    }
    document.addEventListener('mousedown', handler)
    onCleanup(() => document.removeEventListener('mousedown', handler))
  })

  return (
    <div ref={ref} class="relative">
      <button
        onClick={() => {
          if (open()) setConfirmAction(null)
          setOpen(!open())
        }}
        class="text-text-tertiary hover:text-text-primary px-2 py-1 text-lg leading-none"
      >
        &#x22EE;
      </button>
      <Show when={open()}>
        <div class="absolute right-0 mt-1 w-44 bg-container-bg border border-container-border z-10 flex flex-col">
          <button
            onClick={() => {
              props.onTest()
              setOpen(false)
            }}
            class="text-left px-4 py-2.5 text-xs font-mono text-text-secondary hover:bg-main-bg transition-colors"
          >
            {props.isTesting ? 'TESTING...' : 'TEST CONNECTION'}
          </button>
          <Show when={props.showRevoke}>
            <Show
              when={confirmAction() === 'revoke'}
              fallback={
                <button
                  onClick={() => setConfirmAction('revoke')}
                  class="text-left px-4 py-2.5 text-xs font-mono text-signal-red hover:bg-signal-red/10 border-t border-container-border transition-colors"
                >
                  REVOKE AGENT
                </button>
              }
            >
              <div class="flex border-t border-container-border">
                <button
                  onClick={() => {
                    props.onRevoke()
                    setOpen(false)
                    setConfirmAction(null)
                  }}
                  class="flex-1 px-4 py-2.5 text-xs font-mono text-signal-red hover:bg-signal-red/10 transition-colors"
                >
                  CONFIRM
                </button>
                <button
                  onClick={() => setConfirmAction(null)}
                  class="px-4 py-2.5 text-xs font-mono text-text-tertiary hover:bg-main-bg border-l border-container-border transition-colors"
                >
                  NO
                </button>
              </div>
            </Show>
          </Show>
          <Show
            when={confirmAction() === 'delete'}
            fallback={
              <button
                onClick={() => setConfirmAction('delete')}
                class="text-left px-4 py-2.5 text-xs font-mono text-signal-red hover:bg-signal-red/10 border-t border-container-border transition-colors"
              >
                DELETE
              </button>
            }
          >
            <div class="flex border-t border-container-border">
              <button
                onClick={() => {
                  props.onDelete()
                  setOpen(false)
                  setConfirmAction(null)
                }}
                class="flex-1 px-4 py-2.5 text-xs font-mono text-signal-red hover:bg-signal-red/10 transition-colors"
              >
                CONFIRM
              </button>
              <button
                onClick={() => setConfirmAction(null)}
                class="px-4 py-2.5 text-xs font-mono text-text-tertiary hover:bg-main-bg border-l border-container-border transition-colors"
              >
                NO
              </button>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  )
}

function formatBalance(balance?: ExchangeBalanceResponse): string | null {
  if (!balance || balance.balances.length === 0) return null
  const primary = balance.balances.find((b) => b.asset === 'USDT' || b.asset === 'USDC')
    || balance.balances[0]
  const total = parseFloat(primary.total)
  if (isNaN(total)) return null
  return `$${total.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`
}

interface ExchangeCardProps {
  account: ExchangeAccount
  testResult?: TestConnectionResult
  balance?: ExchangeBalanceResponse
  isTesting: boolean
  isDeleting: boolean
  isRevoking: boolean
  onTest: () => void
  onDelete: () => void
  onRevoke: () => void
  onMigrate: () => void
}

export function ExchangeCard(props: ExchangeCardProps) {
  const isAgentWallet = () => props.account.auth_mode === 'agent_wallet'
  const walletAddr = () => props.account.agent_wallet_address

  return (
    <div class="border border-container-border bg-container-bg p-5 flex flex-col gap-4">
      {/* Header: heartbeat + name + badge + kebab */}
      <div class="flex justify-between items-start">
        <div class="flex items-center gap-3">
          <span
            class={`inline-block w-2.5 h-2.5 rounded-full ${
              props.account.is_active
                ? 'bg-signal-green animate-pulse'
                : 'bg-signal-red'
            }`}
          />
          <h3 class="font-mono text-sm font-bold text-text-primary tracking-wider uppercase">
            {props.account.exchange_name}
          </h3>
          <span class="text-[10px] text-text-tertiary font-mono bg-main-bg px-2 py-0.5 border border-container-border">
            {isAgentWallet() ? 'DEX' : 'CEX'}
          </span>
        </div>
        <KebabMenu
          onTest={props.onTest}
          onDelete={props.onDelete}
          onRevoke={props.onRevoke}
          showRevoke={isAgentWallet()}
          isTesting={props.isTesting}
        />
      </div>

      {/* Identifier */}
      <span class="text-xs text-text-tertiary font-mono truncate">
        {walletAddr()
          ? `${walletAddr()!.slice(0, 6)}...${walletAddr()!.slice(-4)}`
          : props.account.account_name}
      </span>

      {/* Migration prompt for direct-key Hyperliquid */}
      <Show when={props.account.exchange_name === 'hyperliquid' && !isAgentWallet()}>
        <button
          onClick={props.onMigrate}
          class="text-[10px] font-mono text-signal-amber hover:underline text-left"
        >
          Migrate to agent wallet &rarr;
        </button>
      </Show>

      {/* Balance / test result */}
      <div class="mt-auto">
        <div class="font-mono text-xl text-text-primary">
          {formatBalance(props.balance) || '---'}
        </div>
        <Show when={props.testResult}>
          {(result) => (
            <div class="font-mono text-xs mt-1">
              <Show
                when={result().success}
                fallback={<span class="text-signal-red">{result().error}</span>}
              >
                <span class="text-signal-green">{result().latency_ms}ms</span>
              </Show>
            </div>
          )}
        </Show>
      </div>
    </div>
  )
}
