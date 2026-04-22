import { createResource, For, Show } from 'solid-js'
import { A } from '@solidjs/router'
import { fetchDignitasMe, type DignitasCurrent } from '../api/client'
import { HelpTip } from '../components/HelpTip'
import { HELP } from '../lib/help-content'
import { useAuth } from '../context/AuthContext'

interface InputRow {
  key: keyof DignitasCurrent['contributions']
  label: string
  weight: number
  inverted: boolean
  helpKey: string
}

const INPUTS: InputRow[] = [
  {
    key: 'drawdown_adherence',
    label: 'DRAWDOWN',
    weight: 0.25,
    inverted: false,
    helpKey: 'dignitas.inputs.drawdown',
  },
  {
    key: 'risk_per_trade_consistency',
    label: 'SIZING',
    weight: 0.20,
    inverted: false,
    helpKey: 'dignitas.inputs.risk_consistency',
  },
  {
    key: 'setup_adherence',
    label: 'SETUP',
    weight: 0.20,
    inverted: false,
    helpKey: 'dignitas.inputs.setup_adherence',
  },
  {
    key: 'coach_severity_penalty',
    label: 'COACH',
    weight: 0.20,
    inverted: true,
    helpKey: 'dignitas.inputs.coach_alignment',
  },
  {
    key: 'journal_consistency',
    label: 'JOURNAL',
    weight: 0.15,
    inverted: false,
    helpKey: 'dignitas.inputs.journal_consistency',
  },
]

function pct(raw: string, inverted: boolean): number {
  const v = parseFloat(raw)
  const effective = inverted ? 1 - v : v
  return Math.round(effective * 100)
}

function weightedPoints(raw: string, weight: number, inverted: boolean): string {
  const v = parseFloat(raw)
  const effective = inverted ? 1 - v : v
  return (weight * effective * 100).toFixed(1)
}

function barColor(p: number): string {
  if (p >= 80) return 'bg-signal-green'
  if (p >= 50) return 'bg-text-secondary'
  return 'bg-signal-red'
}

export default function Dignitas() {
  const auth = useAuth()
  const [data] = createResource(
    () => auth.isAuthenticated() || undefined,
    () => fetchDignitasMe(),
  )

  return (
    <div class="flex flex-col h-full overflow-y-auto">
      {/* Header */}
      <div class="px-8 py-5 shrink-0 border-b border-container-border/50 bg-container-bg flex items-center gap-4">
        <h1 class="font-display text-lg font-bold tracking-wider">
          DIGNITAS
          <HelpTip text={HELP['dignitas.transparency'] ?? ''} position="below" />
        </h1>
        <span class="flex-1" />
        <A
          href="/"
          class="font-mono text-[10px] tracking-wider text-text-tertiary hover:text-text-secondary transition-colors"
        >
          ← OVERVIEW
        </A>
      </div>

      {/* Body */}
      <div class="flex-1 min-h-0 max-w-4xl mx-auto w-full px-8 py-8 flex flex-col gap-8">
        <Show
          when={!data.loading}
          fallback={
            <p class="font-mono text-xs text-text-tertiary">LOADING...</p>
          }
        >
          <Show
            when={data()}
            fallback={
              <section class="border border-container-border bg-container-bg px-6 py-8">
                <p class="font-mono text-xs tracking-wider text-text-tertiary uppercase mb-3">
                  // AUTH REQUIRED
                </p>
                <p class="font-display text-sm text-text-secondary">
                  Sign in to view your Dignitas score breakdown.
                </p>
              </section>
            }
          >
            {(d) => (
              <>
                {/* Score summary */}
                <section class="border border-container-border bg-container-bg px-6 py-6">
                  <p class="font-mono text-[10px] tracking-widest text-text-tertiary mb-4">
                    // CURRENT_SCORE
                  </p>
                  <div class="flex items-baseline gap-4">
                    <span class="font-mono text-4xl text-text-primary">
                      {parseFloat(d().score).toFixed(1)}
                    </span>
                    <Show when={!d().cold_start && d().delta_7d !== null}>
                      <span
                        class={`font-mono text-sm ${
                          parseFloat(d().delta_7d!) > 0
                            ? 'text-signal-green'
                            : parseFloat(d().delta_7d!) < 0
                            ? 'text-signal-red'
                            : 'text-text-tertiary'
                        }`}
                      >
                        {parseFloat(d().delta_7d!) > 0 ? '▲' : '▼'}
                        {Math.abs(parseFloat(d().delta_7d!)).toFixed(1)} vs 7d
                      </span>
                    </Show>
                    <Show when={d().cold_start}>
                      <span class="font-mono text-xs text-text-tertiary">
                        NEUTRAL — BUILDING BASELINE
                        <HelpTip text={HELP['dignitas.cold_start'] ?? ''} />
                      </span>
                    </Show>
                  </div>
                </section>

                {/* Input breakdown */}
                <section class="border border-container-border bg-container-bg">
                  <div class="px-6 py-4 border-b border-container-border">
                    <p class="font-mono text-[10px] tracking-widest text-text-tertiary">
                      // INPUT_BREAKDOWN
                      <HelpTip text={HELP['dignitas.transparency'] ?? ''} />
                    </p>
                  </div>

                  {/* Table header */}
                  <div class="grid grid-cols-[1fr_80px_60px_80px] gap-x-4 px-6 py-2 border-b border-container-border/50">
                    <span class="font-mono text-[9px] tracking-widest text-text-tertiary uppercase">Input</span>
                    <span class="font-mono text-[9px] tracking-widest text-text-tertiary uppercase text-right">Value</span>
                    <span class="font-mono text-[9px] tracking-widest text-text-tertiary uppercase text-right">Weight</span>
                    <span class="font-mono text-[9px] tracking-widest text-text-tertiary uppercase text-right">Points</span>
                  </div>

                  <For each={INPUTS}>
                    {(row) => {
                      const p = pct(d().contributions[row.key], row.inverted)
                      const pts = weightedPoints(d().contributions[row.key], row.weight, row.inverted)
                      return (
                        <div class="px-6 py-4 border-b border-container-border/30 last:border-b-0">
                          <div class="grid grid-cols-[1fr_80px_60px_80px] gap-x-4 items-center mb-2">
                            <span class="font-mono text-xs text-text-primary flex items-center gap-1">
                              {row.label}
                              <HelpTip text={HELP[row.helpKey] ?? ''} />
                            </span>
                            <span class="font-mono text-xs text-text-secondary text-right">{p}%</span>
                            <span class="font-mono text-xs text-text-tertiary text-right">
                              {Math.round(row.weight * 100)}%
                            </span>
                            <span class="font-mono text-xs text-text-secondary text-right">{pts}</span>
                          </div>
                          {/* Progress bar */}
                          <div class="h-0.5 bg-text-primary/5 col-span-4">
                            <div
                              class={`h-full transition-all ${barColor(p)}`}
                              style={{ width: `${p}%` }}
                            />
                          </div>
                        </div>
                      )
                    }}
                  </For>
                </section>

                {/* Formula */}
                <section class="border border-container-border bg-container-bg">
                  <div class="px-6 py-4 border-b border-container-border">
                    <p class="font-mono text-[10px] tracking-widest text-text-tertiary">
                      // FORMULA
                    </p>
                  </div>
                  <div class="px-6 py-5">
                    <pre class="font-mono text-xs text-text-secondary leading-relaxed whitespace-pre-wrap">
{`score = 100 × (
  0.25 × drawdown_adherence
+ 0.20 × risk_per_trade_consistency
+ 0.20 × setup_adherence
+ 0.20 × (1 − coach_severity_penalty)
+ 0.15 × journal_consistency
)

All inputs are normalized to [0, 1].
Weights are tunable in dignitas_config without redeploy.
Historical snapshots reflect the weights in effect at snapshot time.`}
                    </pre>
                  </div>
                </section>

                {/* Ungameability notice */}
                <section class="border border-container-border/40 bg-container-bg px-6 py-4">
                  <p class="font-mono text-[10px] text-text-tertiary leading-relaxed">
                    Trade frequency, raw P&L, and win rate are not inputs to this score.
                    Trading more, trading less, or trading bigger cannot raise it — only adherence to disciplined risk behavior can.
                  </p>
                </section>
              </>
            )}
          </Show>
        </Show>
      </div>
    </div>
  )
}
