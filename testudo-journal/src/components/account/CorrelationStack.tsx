import { For, Show } from 'solid-js'
import type { CorrelationBucket, RiskSnapshot } from '../../api/client'
import { HelpTip } from '../HelpTip'
import { HELP } from '../../lib/help-content'
import { formatCurrency } from '../../lib/formatters'

interface CorrelationStackProps {
  snapshot: RiskSnapshot
}

function directionClass(direction: CorrelationBucket['direction']): string {
  if (direction === 'long') return 'bg-signal-green'
  if (direction === 'short') return 'bg-signal-red'
  return 'bg-signal-amber'
}

function directionLabelClass(direction: CorrelationBucket['direction']): string {
  if (direction === 'long') return 'text-signal-green'
  if (direction === 'short') return 'text-signal-red'
  return 'text-signal-amber'
}

function stripSign(s: string): string {
  return s.replace(/^\+/, '')
}

export function CorrelationStack(props: CorrelationStackProps) {
  if (props.snapshot.correlation_stack.length < 2) return null

  const buckets = () => props.snapshot.correlation_stack

  const maxNotional = () =>
    buckets().reduce((acc, b) => Math.max(acc, parseFloat(b.effective_notional_usd)), 0)

  function widthPct(bucket: CorrelationBucket): number {
    const max = maxNotional()
    if (max <= 0) return 0
    const pct = (parseFloat(bucket.effective_notional_usd) / max) * 100
    return Math.max(2, Math.min(100, pct))
  }

  return (
    <section aria-label="Correlation stack" class="border border-container-border bg-container-bg">
      <div class="flex items-center gap-2 px-4 py-3 border-b border-container-border/60">
        <span class="font-display text-[10px] font-bold tracking-section text-text-secondary uppercase">
          Correlation Stack
        </span>
        <HelpTip text={HELP['risk.correlation']} />
      </div>

      <div class="py-3 px-4 flex flex-col gap-3">
        <For each={buckets()}>
          {(bucket) => <BucketRow bucket={bucket} widthPct={widthPct(bucket)} />}
        </For>
      </div>
    </section>
  )
}

function BucketRow(props: { bucket: CorrelationBucket; widthPct: number }) {
  const symbolsTitle = () => props.bucket.contributing_symbols.join(', ')

  return (
    <div class="flex flex-col gap-1" title={symbolsTitle()}>
      <div class="flex items-center gap-2">
        <span class="font-display text-xs font-bold tracking-wider text-text-primary uppercase">
          {props.bucket.bucket}
        </span>
        <span
          class={`font-mono text-[10px] uppercase tracking-wider ${directionLabelClass(props.bucket.direction)}`}
        >
          {props.bucket.direction}
        </span>
        <span class="flex-1" />
        <span class="font-mono text-xs font-bold text-text-primary">
          {stripSign(formatCurrency(props.bucket.effective_notional_usd))}
        </span>
      </div>
      <div class="h-1.5 bg-text-primary/5 w-full">
        <div
          class={`h-full ${directionClass(props.bucket.direction)}`}
          style={{ width: `${props.widthPct}%` }}
        />
      </div>
      <Show when={props.bucket.contributing_symbols.length > 0}>
        <span class="font-mono text-[10px] text-text-tertiary tracking-wider">
          {props.bucket.contributing_symbols.join(' · ')}
        </span>
      </Show>
    </div>
  )
}
