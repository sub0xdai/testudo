/** @anchor ui:journal:ExchangeCard
 * @tags ui */

import { createSignal, createEffect, onCleanup, For, Show } from 'solid-js'
import type {
  ExchangeAccount,
  TestConnectionResult,
  RiskSnapshot,
  VenueMargin,
  PositionEntry,
} from '../../api/client'
import { formatCurrency, formatNumber, formatPrice, pnlColor } from '../../lib/formatters'
import { exchangeApi } from '../../api/client'

interface KebabMenuProps {
  onTest: () => void
  onDelete: () => void
  onRevoke: () => void
  onImport: () => void
  showRevoke: boolean
  showImport: boolean
  isTesting: boolean
  isImporting: boolean
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
        aria-label="Account options"
        aria-expanded={open()}
        onClick={() => {
          if (open()) setConfirmAction(null)
          setOpen(!open())
        }}
        class="btn-ghost text-text-tertiary hover:text-text-primary px-2 py-1 text-lg leading-none"
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
            class="btn-ghost text-left px-4 py-2.5 text-text-secondary hover:bg-main-bg transition-colors"
          >
            {props.isTesting ? 'TESTING...' : 'TEST CONNECTION'}
          </button>
          <Show when={props.showImport}>
            <button
              onClick={() => {
                props.onImport()
                setOpen(false)
              }}
              disabled={props.isImporting}
              class="btn-ghost text-left px-4 py-2.5 border-t border-container-border transition-colors disabled:opacity-50 text-text-secondary hover:bg-main-bg"
            >
              {props.isImporting ? 'IMPORTING...' : 'IMPORT'}
            </button>
          </Show>
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
                  class="btn-destructive flex-1 px-4 py-2.5 border-0"
                >
                  CONFIRM
                </button>
                <button
                  onClick={() => setConfirmAction(null)}
                  class="btn-ghost px-4 py-2.5 text-text-tertiary hover:bg-main-bg border-l border-container-border transition-colors"
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
                class="btn-destructive flex-1 px-4 py-2.5 border-0"
              >
                CONFIRM
              </button>
              <button
                onClick={() => setConfirmAction(null)}
                class="btn-ghost px-4 py-2.5 text-text-tertiary hover:bg-main-bg border-l border-container-border transition-colors"
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

function formatBalanceUsd(raw: string): string {
  const num = parseFloat(raw)
  if (isNaN(num)) return '$0.00'
  return `$${Math.abs(num).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`
}

function freeRatio(margin: VenueMargin): number {
  const total = parseFloat(margin.total_usd)
  const free = parseFloat(margin.free_usd)
  if (!isFinite(total) || total <= 0) return 0
  const pct = (free / total) * 100
  if (!isFinite(pct)) return 0
  return Math.max(0, Math.min(100, pct))
}

function PositionsSection(props: { positions: PositionEntry[] }) {
  const count = () => props.positions.length
  return (
    <div class="flex flex-col gap-2 pt-3 border-t border-container-border/50">
      <Show
        when={count() > 0}
        fallback={
          <div class="font-mono text-[10px] tracking-widest text-text-tertiary uppercase">
            ── NO OPEN POSITIONS ──
          </div>
        }
      >
        <div class="font-mono text-[10px] tracking-widest text-text-tertiary uppercase">
          ── {count()} {count() === 1 ? 'POSITION' : 'POSITIONS'} ──
        </div>
        <div class="flex flex-col gap-2">
          <For each={props.positions}>{(pos) => <PositionRow pos={pos} />}</For>
        </div>
      </Show>
    </div>
  )
}

function PositionRow(props: { pos: PositionEntry }) {
  const sideClass = () =>
    props.pos.side === 'long' ? 'text-signal-green' : 'text-signal-red'
  return (
    <div class="flex flex-col gap-0.5">
      <div class="flex items-center gap-2">
        <span class="font-mono text-xs text-text-primary">{props.pos.symbol}</span>
        <span class="font-mono text-[10px] text-text-tertiary">·</span>
        <span class={`font-mono text-[10px] uppercase tracking-wider ${sideClass()}`}>
          {props.pos.side}
        </span>
      </div>
      <div class="font-mono text-[10px] text-text-tertiary">
        {formatNumber(props.pos.quantity, 4)} @ {formatPrice(props.pos.entry_price)} &rarr;{' '}
        {formatPrice(props.pos.mark_price)}
      </div>
      <div class={`font-mono text-[10px] ${pnlColor(props.pos.unrealized_pnl_usd)}`}>
        {formatCurrency(props.pos.unrealized_pnl_usd)}
      </div>
    </div>
  )
}

interface ExchangeCardProps {
  account: ExchangeAccount
  testResult?: TestConnectionResult
  snapshot?: RiskSnapshot
  isTesting: boolean
  isDeleting: boolean
  isRevoking: boolean
  onTest: () => void
  onDelete: () => void
  onRevoke: () => void
  onMigrate: () => void
  onReauthorize: () => void
  onImport: () => void
  isImporting: boolean
}

export function ExchangeCard(props: ExchangeCardProps) {
  const isAgentWallet = () => props.account.auth_mode === 'agent_wallet'
  const isHlAgent = () => props.account.exchange_name === 'hyperliquid' && isAgentWallet()
  
  // Live balance for Hyperliquid agent wallets
  const [liveBalance, setLiveBalance] = createSignal<{ spot: string; perp: string } | null>(null)
  createEffect(() => {
    const hl = isHlAgent()
    const id = props.account.id
    console.log('[ExchangeCard] effect running:', { hl, id, authMode: props.account.auth_mode })
    if (!hl || !id) return
    exchangeApi.fetchBalance(id).then(b => {
      console.log('[ExchangeCard] balance ok:', b)
      setLiveBalance({
        spot: b.balances.find(x => x.asset === 'USDC (Spot)')?.total ?? '0',
        perp: b.balances.find(x => x.asset === 'USDC (Perp)')?.total ?? '0',
      })
    }).catch(e => { console.error('[ExchangeCard] balance fail:', e) })
  })
  const walletAddr = () => props.account.agent_wallet_address
  const needsReauth = () => props.account.requires_reauthorization === true
  const venueMargin = (): VenueMargin | undefined =>
    props.snapshot?.margin_by_venue.find((m) => m.exchange_id === props.account.id)
  const venuePositions = (): PositionEntry[] =>
    props.snapshot?.positions_by_venue.find((v) => v.exchange_id === props.account.id)
      ?.positions ?? []

  return (
    <div class={`border ${
      needsReauth()
        ? 'border-signal-amber bg-signal-amber/5'
        : 'border-container-border bg-container-bg'
    } p-10 flex flex-col gap-6 min-h-[280px]`}>
      {/* Header: heartbeat + name + badge + kebab */}
      <div class="flex justify-between items-start">
        <div class="flex items-center gap-3">
          <span
            class={`inline-block w-2.5 h-2.5 rounded-full ${
              needsReauth()
                ? 'bg-signal-amber animate-pulse'
                : props.account.is_active
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
          <Show when={needsReauth()}>
            <span class="text-[10px] text-signal-amber font-mono bg-signal-amber/10 px-2 py-0.5 border border-signal-amber/30">
              REAUTH REQUIRED
            </span>
          </Show>
        </div>
        <KebabMenu
          onTest={props.onTest}
          onDelete={props.onDelete}
          onRevoke={props.onRevoke}
          onImport={props.onImport}
          showRevoke={isAgentWallet()}
          showImport={true}
          isTesting={props.isTesting}
          isImporting={props.isImporting}
        />
      </div>

      {/* Identifier */}
      <span class="text-xs text-text-tertiary font-mono truncate">
        {walletAddr()
          ? `${walletAddr()!.slice(0, 6)}...${walletAddr()!.slice(-4)}`
          : props.account.account_name}
      </span>

      {/* Migration prompt for direct-key Hyperliquid */}
      <Show when={props.account.exchange_name === 'hyperliquid' && !isAgentWallet() && !needsReauth()}>
        <button
          onClick={props.onMigrate}
          class="text-[10px] font-mono text-signal-amber hover:underline text-left"
        >
          Migrate to agent wallet &rarr;
        </button>
      </Show>

      {/* Margin breakdown / test result — or reauth button when degraded */}
      <div class="mt-auto flex flex-col gap-2">
        <Show when={needsReauth()} fallback={
          <>
            <Show
              when={venueMargin()}
              fallback={
                <div class="font-mono text-xs text-text-tertiary">Margin unavailable</div>
              }
            >
              {(m) => (
                <div class="flex flex-col gap-1.5">
                  {/* Live balance for HL agent wallets, snapshot for everything else */}
                  <Show when={isHlAgent() && liveBalance()} fallback={
                    <div class="flex items-baseline gap-2">
                      <span class="font-mono text-2xl font-bold text-text-primary">
                        {formatBalanceUsd(m().total_usd)}
                      </span>
                      <span class="font-mono text-[10px] uppercase tracking-wider text-text-tertiary">
                        total
                      </span>
                    </div>
                  }>
                    {(b) => (
                      <>
                        <div class="flex items-baseline gap-2">
                          <span class="font-mono text-2xl font-bold text-text-primary">
                            {formatBalanceUsd(String(parseFloat(b().spot) + parseFloat(b().perp)))}
                          </span>
                          <span class="font-mono text-[10px] uppercase tracking-wider text-text-tertiary">
                            total
                          </span>
                        </div>
                        <div class="flex gap-4 font-mono text-[10px] text-text-tertiary">
                          <span>Spot <span class="text-text-primary">{formatBalanceUsd(b().spot)}</span></span>
                          <span>Perp <span class="text-text-primary">{formatBalanceUsd(b().perp)}</span></span>
                        </div>
                      </>
                    )}
                  </Show>
                  <div class="h-1.5 bg-text-primary/5 w-full">
                    <div
                      class="h-full bg-signal-green"
                      style={{ width: `${freeRatio(m())}%` }}
                    />
                  </div>
                  <div class="font-mono text-[10px] text-text-tertiary tracking-wider uppercase">
                    {formatBalanceUsd(m().free_usd)} free · {formatBalanceUsd(m().used_usd)} used
                  </div>

                </div>
              )}
            </Show>
            <PositionsSection positions={venuePositions()} />
            <Show when={props.testResult}>
              {(result) => (
                <div class="font-mono text-xs">
                  <Show
                    when={result().success}
                    fallback={<span class="text-signal-red">{result().error}</span>}
                  >
                    <span class="text-signal-green">{result().latency_ms}ms</span>
                  </Show>
                </div>
              )}
            </Show>
          </>
        }>
          <button
            onClick={() => props.onReauthorize()}
            class="w-full py-3 border border-signal-amber text-signal-amber font-mono font-bold text-xs tracking-wider hover:bg-signal-amber hover:text-main-bg transition-colors"
          >
            [ REAUTHORIZE ]
          </button>
        </Show>
      </div>
    </div>
  )
}
