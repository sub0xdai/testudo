import { For, Show } from 'solid-js'
import type { JournalTrade, JournalTag } from '../../api/client'
import { formatCurrency, formatPrice, formatDuration, formatDate, pnlColor, rColor, sideColor } from '../../lib/formatters'

export function TradeRow(props: {
  trade: JournalTrade
  tags?: JournalTag[]
  onClick: () => void
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
      class="border-b border-container-border/30 hover:bg-elevated cursor-pointer transition-colors even:bg-white/[0.02]"
      onClick={props.onClick}
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
        <Show when={props.tags && props.tags.length > 0}>
          <div class="flex gap-1">
            <For each={props.tags!.slice(0, 2)}>
              {(tag, i) => (
                <span
                  class="inline-flex items-center gap-0.5 px-1.5 py-0 text-[10px] font-mono border rounded"
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
      </td>
    </tr>
  )
}
