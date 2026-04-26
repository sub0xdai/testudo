import { For, Show } from 'solid-js'
import type { JournalTrade, JournalTag } from '../../api/client'
import { formatCurrency, formatPrice, formatDuration, formatDate, pnlColor, rColor, sideColor } from '../../lib/formatters'

export function TradeRow(props: {
  trade: JournalTrade
  tags?: JournalTag[]
  onClick: () => void
  onKellyBadgeClick?: () => void
}) {
  const t = () => props.trade
  const rMultiple = () => {
    const r = t().r_multiple
    if (!r) return '—'
    const num = parseFloat(r)
    return `${num >= 0 ? '+' : ''}${num.toFixed(1)}R`
  }

  return (
    <tr
      tabIndex={0}
      role="button"
      class="border-b border-container-border/30 hover:bg-elevated focus:bg-elevated cursor-pointer transition-colors even:bg-text-primary/[0.02] outline-none"
      onClick={props.onClick}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          props.onClick()
        }
      }}
    >
      <td class="px-3 py-2.5 text-xs font-mono text-text-secondary whitespace-nowrap">
        {formatDate(t().closed_at)}
      </td>
      <td class="px-3 py-2.5 text-xs font-mono text-text-primary whitespace-nowrap">
        {t().symbol.replace('_', '')}
      </td>
      <td class="px-3 py-2.5 text-xs font-mono text-text-secondary whitespace-nowrap uppercase">
        {t().exchange.slice(0, 3)}
      </td>
      <td class={`px-3 py-2.5 text-xs font-mono whitespace-nowrap uppercase ${sideColor(t().side)}`}>
        {t().side.toLowerCase() === 'long' ? 'LONG' : 'SHRT'}
      </td>
      <td class="px-3 py-2.5 text-xs font-mono text-text-primary whitespace-nowrap text-right">
        {formatPrice(t().entry_price)}
      </td>
      <td class="px-3 py-2.5 text-xs font-mono text-text-primary whitespace-nowrap text-right">
        {formatPrice(t().exit_price)}
      </td>
      <td class={`px-3 py-2.5 text-xs font-mono whitespace-nowrap text-right ${pnlColor(t().net_pnl)}`}>
        {formatCurrency(t().net_pnl)}
      </td>
      <td class={`px-3 py-2.5 text-xs font-mono whitespace-nowrap text-right ${rColor(t().r_multiple)}`}>
        {rMultiple()}
      </td>
      <td class="px-3 py-2.5 text-xs font-mono text-text-secondary whitespace-nowrap text-right">
        {formatDuration(t().duration_secs)}
      </td>
      <td class="px-3 py-2.5 whitespace-nowrap">
        <div class="flex items-center gap-1.5">
          <Show when={t().kelly_inputs != null}>
            <button
              class="inline-flex items-center gap-0.5 px-1 py-0 text-[10px] font-mono border border-signal-green/40 text-signal-green/80 hover:border-signal-green hover:text-signal-green transition-colors"
              title="Kelly-sized — click to see calibration inputs"
              onClick={(e) => {
                e.stopPropagation()
                props.onKellyBadgeClick?.()
              }}
            >
              ⚡ Kelly
            </button>
          </Show>
          <Show when={t().notes}>
            <span class="text-text-tertiary" title="Has notes">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 20h9" /><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
              </svg>
            </span>
          </Show>
          <Show when={props.tags && props.tags.length > 0}>
            <div class="flex gap-1">
              <For each={props.tags!.slice(0, 2)}>
                {(tag) => (
                  <span
                    class="inline-flex items-center gap-0.5 px-1.5 py-0 text-[10px] font-mono border"
                    style={{
                      'border-color': tag.color || '#555',
                      color: tag.color || '#555',
                    }}
                  >
                    <span
                      class="w-1 h-1 rounded-full"
                      style={{ background: tag.color || '#555' }}
                    />
                    {tag.name}
                  </span>
                )}
              </For>
              <Show when={props.tags!.length > 2}>
                <span class="text-[10px] font-mono text-text-tertiary">
                  +{props.tags!.length - 2}
                </span>
              </Show>
            </div>
          </Show>
        </div>
      </td>
    </tr>
  )
}
