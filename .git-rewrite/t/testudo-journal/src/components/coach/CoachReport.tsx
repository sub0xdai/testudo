import { For, Show } from 'solid-js'
import type {
  StoredCoachReport,
  CoachFlaggedPattern,
  CoachPatternKind,
  CoachSeverity,
} from '../../api/client'
import { NarrativeBlock } from './NarrativeBlock'
import { HelpTip } from '../HelpTip'
import { HELP } from '../../lib/help-content'
import { formatCurrency, formatPercent, pnlColor, formatDateFull } from '../../lib/formatters'

const PATTERN_LABELS: Record<CoachPatternKind, string> = {
  sizing_drift: 'Sizing Drift',
  frequency_spike: 'Frequency Spike',
  session_anomaly: 'Session Anomaly',
  setup_fatigue: 'Setup Fatigue',
  correlation_stack: 'Correlation Stack',
  streak_risk: 'Streak Risk',
}

function severityClass(sev: CoachSeverity): string {
  if (sev === 'concerning') return 'text-signal-red border-signal-red/60'
  if (sev === 'notable') return 'text-signal-amber border-signal-amber/60'
  return 'text-text-secondary border-container-border'
}

interface CoachReportProps {
  report: StoredCoachReport
}

export function CoachReport(props: CoachReportProps) {
  const weekRange = () =>
    `${formatDateFull(props.report.week_start)} → ${formatDateFull(props.report.week_end)}`

  const stats = () => props.report.digest.week_stats
  const flagged = () => props.report.digest.flagged_trades
  const patterns = () => props.report.digest.flagged_patterns
  const narrative = () => props.report.narrative_sections
  const headline = () => props.report.headline

  return (
    <div class="flex flex-col gap-8">
      {/* Deterministic stats block */}
      <section class="border border-container-border bg-container-bg">
        <header class="flex items-center gap-2 px-4 py-3 border-b border-container-border/60">
          <span class="font-display text-[10px] font-bold tracking-section text-text-secondary uppercase">
            Weekly Digest
          </span>
          <span class="flex-1" />
          <span class="font-mono text-[11px] text-text-tertiary tracking-wider">
            {weekRange()}
          </span>
        </header>

        <Show when={headline()}>
          <div class="px-4 py-4 border-b border-container-border/60">
            <p class="font-display text-base text-text-primary leading-snug">
              {headline()}
            </p>
          </div>
        </Show>

        <div class="grid grid-cols-2 md:grid-cols-4 divide-x divide-y md:divide-y-0 divide-container-border">
          <StatCell label="Trades" value={String(stats().trade_count)} />
          <StatCell label="Win Rate" value={formatPercent(stats().win_rate)} />
          <StatCell
            label="Total P&L"
            value={formatCurrency(stats().total_pnl)}
            valueClass={pnlColor(stats().total_pnl)}
          />
          <StatCell label="Total R" value={stats().total_r} />
        </div>

        <Show when={patterns().length > 0}>
          <div class="flex flex-wrap gap-2 px-4 py-3 border-t border-container-border/60">
            <For each={patterns()}>
              {(p) => <PatternBadge pattern={p} />}
            </For>
          </div>
        </Show>
      </section>

      {/* Narrative — either sections or unavailable fallback */}
      <section>
        <header class="flex items-center gap-2 px-4 py-3">
          <span class="font-display text-[10px] font-bold tracking-section text-text-secondary uppercase">
            Coach Narrative
          </span>
          <HelpTip text={HELP['coach.narrative'] ?? ''} />
        </header>
        <Show
          when={narrative() && narrative()!.length > 0}
          fallback={
            <div class="border border-container-border bg-container-bg px-4 py-6">
              <p class="font-mono text-xs tracking-wider text-signal-amber">
                ● coach unavailable this week
              </p>
              <p class="font-display text-sm text-text-secondary mt-2 leading-relaxed">
                The weekly report was generated, but the narrative layer could not be produced.
                Deterministic stats and flagged patterns above remain accurate.
              </p>
            </div>
          }
        >
          <NarrativeBlock sections={narrative()!} flagged={flagged()} />
        </Show>
      </section>

      {/* Metadata */}
      <section class="flex flex-wrap gap-x-6 gap-y-1 font-mono text-[10px] tracking-wider text-text-tertiary uppercase">
        <span>model: {props.report.model_used}</span>
        <span>generated: {formatDateFull(props.report.generated_at)}</span>
        <Show when={props.report.cache_hit_ratio !== null}>
          <span>cache hit: {formatPercent(parseFloat(props.report.cache_hit_ratio!) * 100)}</span>
        </Show>
        <HelpTip text={HELP['coach.provider'] ?? ''} />
      </section>
    </div>
  )
}

function StatCell(props: { label: string; value: string; valueClass?: string }) {
  return (
    <div class="px-4 py-3 flex flex-col gap-1">
      <span class="font-mono text-[10px] tracking-wider text-text-tertiary uppercase">
        {props.label}
      </span>
      <span class={`font-mono text-sm font-bold ${props.valueClass ?? 'text-text-primary'}`}>
        {props.value}
      </span>
    </div>
  )
}

function PatternBadge(props: { pattern: CoachFlaggedPattern }) {
  const label = () => PATTERN_LABELS[props.pattern.pattern] ?? props.pattern.pattern
  const count = () => props.pattern.evidence.length
  return (
    <span
      class={`inline-flex items-center gap-2 border px-2.5 py-1 font-mono text-[10px] tracking-wider uppercase ${severityClass(props.pattern.severity)}`}
    >
      <span>{label()}</span>
      <span class="text-text-tertiary">·</span>
      <span>{props.pattern.severity}</span>
      <span class="text-text-tertiary">·</span>
      <span>
        {count()} {count() === 1 ? 'trade' : 'trades'}
      </span>
    </span>
  )
}
