import { Show } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import type { RiskSnapshot } from '../api/client'
import { formatCurrency, formatNumber } from '../lib/formatters'

interface PulseStripProps {
  snapshot: RiskSnapshot | null
  isStale?: boolean
}

function stripDollarSign(formatted: string): string {
  return formatted.replace(/^\+/, '')
}

export function PulseStrip(props: PulseStripProps) {
  const navigate = useNavigate()

  const exposure = () => (props.snapshot ? stripDollarSign(formatCurrency(props.snapshot.net_exposure_usd)) : '$0.00')
  const leverage = () => (props.snapshot ? `${formatNumber(props.snapshot.aggregate_leverage, 1)}x` : '0.0x')
  const freeMargin = () => (props.snapshot ? stripDollarSign(formatCurrency(props.snapshot.free_margin_usd)) : '$0.00')

  return (
    <button
      type="button"
      onClick={() => navigate('/account')}
      aria-label="View risk snapshot"
      class="w-full shrink-0 bg-main-bg border-b border-container-border hover:bg-container-bg transition-colors"
    >
      <div class="flex items-center justify-between gap-4 px-6 md:px-8 h-7">
        <div class="flex items-center gap-2 font-mono text-[10px] tracking-wider text-text-tertiary uppercase">
          <Show
            when={props.isStale}
            fallback={<span class="inline-block w-1.5 h-1.5 rounded-full bg-signal-green animate-pulse" aria-hidden="true" />}
          >
            <span class="inline-block w-1.5 h-1.5 rounded-full bg-signal-amber" aria-hidden="true" />
          </Show>
          <span>Pulse</span>
          <Show when={props.isStale}>
            <span class="text-signal-amber">stale</span>
          </Show>
        </div>

        {/* Desktop: full format */}
        <div class="hidden md:flex items-center gap-3 font-mono text-xs text-text-secondary">
          <span class="text-text-primary">{exposure()}</span>
          <span class="text-text-tertiary">&middot;</span>
          <span class="text-text-primary">{leverage()}</span>
          <span class="text-text-tertiary">&middot;</span>
          <span>
            <span class="text-text-primary">{freeMargin()}</span>
            <span class="text-text-tertiary"> free</span>
          </span>
        </div>

        {/* Mobile: compressed */}
        <div class="flex md:hidden items-center gap-2 font-mono text-[11px] text-text-primary">
          <span>{exposure()}</span>
          <span class="text-text-tertiary">/</span>
          <span>{leverage()}</span>
        </div>
      </div>
    </button>
  )
}
