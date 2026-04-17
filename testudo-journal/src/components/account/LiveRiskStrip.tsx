import { For } from 'solid-js'
import type { RiskSnapshot } from '../../api/client'
import { formatCurrency, formatPercent, formatNumber, pnlColor } from '../../lib/formatters'

interface LiveRiskStripProps {
  snapshot: RiskSnapshot
}

interface Metric {
  label: string
  value: string
  colorClass?: string
}

function longShortValue(longPct: string, shortPct: string): string {
  return `${formatPercent(parseFloat(longPct) * 100)} / ${formatPercent(parseFloat(shortPct) * 100)}`
}

export function LiveRiskStrip(props: LiveRiskStripProps) {
  const metrics = (): Metric[] => {
    const s = props.snapshot
    const leverage = parseFloat(s.aggregate_leverage)
    return [
      {
        label: 'NET EXPOSURE',
        value: formatCurrency(s.net_exposure_usd).replace(/^\+/, ''),
        colorClass: pnlColor(s.net_delta_usd),
      },
      {
        label: 'LEVERAGE',
        value: `${formatNumber(s.aggregate_leverage, 1)}x`,
        colorClass: leverage >= 5 ? 'text-signal-red' : leverage >= 2 ? 'text-signal-amber' : 'text-text-primary',
      },
      {
        label: 'FREE MARGIN',
        value: formatCurrency(s.free_margin_usd).replace(/^\+/, ''),
      },
      {
        label: 'LONG / SHORT',
        value: longShortValue(props.snapshot.long_pct, props.snapshot.short_pct),
      },
    ]
  }

  return (
    <div class="border border-container-border bg-container-bg">
      <div class="grid grid-cols-2 md:grid-cols-4 divide-y divide-container-border md:divide-y-0 md:divide-x md:divide-container-border">
        <For each={metrics()}>
          {(m) => (
            <div class="px-6 py-5 flex flex-col gap-2">
              <span class="font-display text-[10px] font-bold tracking-section text-text-secondary uppercase">
                {m.label}
              </span>
              <span class={`font-mono text-xl font-bold ${m.colorClass ?? 'text-text-primary'}`}>
                {m.value}
              </span>
            </div>
          )}
        </For>
      </div>
    </div>
  )
}
