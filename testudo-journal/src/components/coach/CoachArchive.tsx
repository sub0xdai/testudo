/** @anchor ui:journal:CoachArchive
 * @tags ui */

import { For, Show, createSignal } from 'solid-js'
import type { StoredCoachReport, CoachPatternKind } from '../../api/client'
import { CoachReport } from './CoachReport'
import { formatDateFull } from '../../lib/formatters'

const PATTERN_SHORT: Record<CoachPatternKind, string> = {
  sizing_drift: 'SIZING',
  frequency_spike: 'FREQ',
  session_anomaly: 'SESSION',
  setup_fatigue: 'FATIGUE',
  correlation_stack: 'CORR',
  streak_risk: 'STREAK',
}

interface CoachArchiveProps {
  items: StoredCoachReport[]
  onLoadMore?: () => void
  canLoadMore?: boolean
  loading?: boolean
}

export function CoachArchive(props: CoachArchiveProps) {
  return (
    <section class="flex flex-col gap-3">
      <header class="flex items-center gap-2 px-4 py-3">
        <span class="font-display text-[10px] font-bold tracking-section text-text-secondary uppercase">
          Past Reports
        </span>
      </header>

      <Show
        when={props.items.length > 0}
        fallback={
          <p class="px-4 py-6 font-display text-sm text-text-tertiary">
            No past reports yet. New reports land here every Sunday.
          </p>
        }
      >
        <ul class="flex flex-col border border-container-border bg-container-bg divide-y divide-container-border">
          <For each={props.items}>
            {(item) => <ArchiveRow report={item} />}
          </For>
        </ul>
      </Show>

      <Show when={props.onLoadMore && props.canLoadMore}>
        <div class="px-4 py-3">
          <button
            onClick={() => props.onLoadMore?.()}
            disabled={props.loading}
            class="font-mono text-xs tracking-wider px-3 py-1.5 border border-container-border text-text-secondary hover:text-text-primary hover:border-text-secondary disabled:opacity-50 transition-colors"
          >
            {props.loading ? 'LOADING…' : 'LOAD MORE'}
          </button>
        </div>
      </Show>
    </section>
  )
}

function ArchiveRow(props: { report: StoredCoachReport }) {
  const [expanded, setExpanded] = createSignal(false)

  const weekRange = () =>
    `${formatDateFull(props.report.week_start)} → ${formatDateFull(props.report.week_end)}`

  const patterns = () => props.report.digest.flagged_patterns

  return (
    <li>
      <button
        class="w-full flex items-center gap-4 px-4 py-3 text-left hover:bg-text-primary/5 transition-colors"
        onClick={() => setExpanded(!expanded())}
        aria-expanded={expanded()}
      >
        <span class="font-mono text-[11px] tracking-wider text-text-tertiary shrink-0">
          {weekRange()}
        </span>
        <span class="flex-1 font-display text-sm text-text-primary truncate">
          {props.report.headline ?? '(no headline)'}
        </span>
        <div class="hidden md:flex gap-1 shrink-0">
          <For each={patterns()}>
            {(p) => (
              <span class="font-mono text-[9px] tracking-wider px-1.5 py-0.5 border border-container-border text-text-tertiary uppercase">
                {PATTERN_SHORT[p.pattern] ?? p.pattern}
              </span>
            )}
          </For>
        </div>
        <span class="font-mono text-xs text-text-tertiary shrink-0">
          {expanded() ? '−' : '+'}
        </span>
      </button>

      <Show when={expanded()}>
        <div class="px-4 py-5 border-t border-container-border/60 bg-main-bg/40">
          <CoachReport report={props.report} />
        </div>
      </Show>
    </li>
  )
}
