import { createResource, createSignal, onCleanup, Show } from 'solid-js'
import { A } from '@solidjs/router'
import { DignitasSparkline } from './DignitasSparkline'
import { fetchDignitasHistory, fetchIdentity, patchDignitasPreference, type DignitasCurrent } from '../api/client'

interface Props {
  data: DignitasCurrent
  onHide: () => void
  onClose: () => void
}

export function DignitasPanel(props: Props) {
  const [history] = createResource(() => fetchDignitasHistory(90))
  const [identity] = createResource(() => fetchIdentity())
  const [hiding, setHiding] = createSignal(false)
  const [copied, setCopied] = createSignal(false)
  let copyTimeout: ReturnType<typeof setTimeout> | null = null

  onCleanup(() => { if (copyTimeout) clearTimeout(copyTimeout) })

  const canShare = () => {
    const id = identity()
    return id !== undefined && id.handle !== null && id.show_score
  }

  function handleShare() {
    const handle = identity()?.handle
    if (!handle) return
    navigator.clipboard.writeText(`${window.location.origin}/desk/d/${handle}`)
    setCopied(true)
    copyTimeout = setTimeout(() => setCopied(false), 2000)
  }

  async function handleHide() {
    setHiding(true)
    try {
      await patchDignitasPreference(true)
      props.onHide()
    } finally {
      setHiding(false)
    }
  }

  const score = () => parseFloat(props.data.score).toFixed(1)
  const delta = () => {
    if (props.data.cold_start || props.data.delta_7d === null) return null
    return parseFloat(props.data.delta_7d)
  }
  // Cold-start copy is concrete on purpose: a trader needs to know exactly
  // how thin the signal is and what would firm it up. "PRELIMINARY" alone
  // doesn't tell them whether they need 1 more trade or 7.
  const COLD_START_TARGET_TRADES = 10
  const preliminaryCopy = () =>
    `PRELIMINARY — ${props.data.trade_count_30d} of ${COLD_START_TARGET_TRADES} trades`
  const deltaColor = () => {
    const d = delta()
    if (d === null) return 'text-text-tertiary'
    if (d > 0) return 'text-signal-green'
    if (d < 0) return 'text-signal-red'
    return 'text-text-tertiary'
  }

  return (
    <div class="p-4">
      <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-4">
        // DIGNITAS_SCORE
      </div>

      {/* Score */}
      <div class="text-center mb-3">
        <div class="font-mono text-3xl text-text-primary">{score()}</div>
        <Show
          when={delta() !== null}
          fallback={
            <div class="font-mono text-[10px] text-text-tertiary mt-1">
              {props.data.cold_start ? preliminaryCopy() : '—'}
            </div>
          }
        >
          <div class={`font-mono text-xs mt-1 ${deltaColor()}`}>
            {delta()! > 0 ? '▲' : '▼'}
            {Math.abs(delta()!).toFixed(1)} vs 7d
          </div>
        </Show>
      </div>

      {/* 90-day sparkline */}
      <Show
        when={!history.loading}
        fallback={
          <div class="h-[80px] flex items-center justify-center">
            <div class="font-mono text-[10px] text-text-tertiary">LOADING...</div>
          </div>
        }
      >
        <DignitasSparkline snapshots={history()?.snapshots ?? []} />
      </Show>
      <div class="font-mono text-[10px] text-text-tertiary text-right mt-1 mb-3">90d</div>

      {/* Streak (ENG-01c) — silent increment, silent reset */}
      <div class="flex items-center justify-between font-mono text-[10px] text-text-tertiary mb-3">
        <Show
          when={props.data.streak}
          fallback={<span>STREAK —</span>}
        >
          {(streak) => (
            <>
              <span class="text-text-secondary">STREAK {streak().days_clean}d</span>
              <span>LONGEST {streak().longest_ever}d</span>
            </>
          )}
        </Show>
      </div>

      {/* Actions */}
      <div class="border-t border-container-border pt-3 flex items-center justify-between">
        <button
          onClick={handleHide}
          disabled={hiding()}
          class="font-mono text-[10px] text-text-tertiary hover:text-text-secondary transition-colors disabled:opacity-50"
        >
          {hiding() ? 'HIDING...' : 'HIDE PILL'}
        </button>
        <div class="flex items-center gap-3">
          <Show when={canShare()}>
            <button
              onClick={handleShare}
              class="font-mono text-[10px] text-text-tertiary hover:text-text-secondary transition-colors"
              classList={{ 'text-signal-green': copied() }}
            >
              {copied() ? 'COPIED!' : 'SHARE PROFILE'}
            </button>
          </Show>
          <A
            href="/dignitas"
            onClick={props.onClose}
            class="font-mono text-[10px] text-text-secondary hover:text-text-primary transition-colors"
          >
            VIEW BREAKDOWN →
          </A>
        </div>
      </div>
    </div>
  )
}
