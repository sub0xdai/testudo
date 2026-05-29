/** @anchor ui:journal:ActivePositions
 * @tags ui */

import { createResource, For, Show, onCleanup } from 'solid-js'
import { fetchActivePositions, type ActivePosition } from '../../api/client'

interface ActivePositionsProps {
  onSelectTrade: (groupId: string) => void
}

export function ActivePositions(props: ActivePositionsProps) {
  const [positions, { refetch }] = createResource(fetchActivePositions)

  // Auto-refresh every 30s
  const interval = setInterval(() => refetch(), 30000)
  onCleanup(() => clearInterval(interval))

  const formatTime = (iso: string) => {
    const diff = Date.now() - new Date(iso).getTime()
    const mins = Math.floor(diff / 60000)
    if (mins < 60) return `${mins}m`
    const hrs = Math.floor(mins / 60)
    if (hrs < 24) return `${hrs}h`
    return `${Math.floor(hrs / 24)}d`
  }

  return (
    <Show when={!positions.loading && positions()?.length}>
      <div class="mb-6">
        <div class="flex items-center gap-2 mb-3 px-1">
          <span class="w-2 h-2 rounded-full bg-signal-green animate-pulse" />
          <span class="text-[10px] font-display font-medium tracking-widest uppercase text-text-tertiary">
            ACTIVE
          </span>
          <span class="text-[10px] font-mono text-text-tertiary bg-text-primary/10 px-1.5 py-0.5">
            {positions()!.length}
          </span>
        </div>
        <div class="border border-container-border bg-container-bg">
          <table class="w-full">
            <tbody>
              <For each={positions()}>
                {(pos: ActivePosition) => (
                  <tr
                    class="border-b border-container-border/30 hover:bg-elevated cursor-pointer transition-colors"
                    onClick={() => props.onSelectTrade(pos.id)}
                  >
                    <td class="px-3 py-2.5 text-xs font-mono text-text-primary whitespace-nowrap">
                      {(pos.symbol || '').replace('_', '')}
                    </td>
                    <td class={`px-3 py-2.5 text-xs font-mono whitespace-nowrap uppercase ${
                      pos.side === 'buy' ? 'text-signal-green' : 'text-signal-red'
                    }`}>
                      {pos.side === 'buy' ? 'LONG' : 'SHORT'}
                    </td>
                    <td class="px-3 py-2.5 text-xs font-mono text-text-secondary whitespace-nowrap">
                      Entry: {parseFloat(pos.entry_price || '0').toLocaleString()}
                    </td>
                    <td class="px-3 py-2.5 text-xs font-mono whitespace-nowrap">
                      <span class="text-signal-green animate-pulse">
                        {'● '}{(pos.status || 'active').toUpperCase()}
                      </span>
                    </td>
                    <td class="px-3 py-2.5 text-xs font-mono text-text-tertiary whitespace-nowrap text-right">
                      {formatTime(pos.created_at)}
                    </td>
                  </tr>
                )}
              </For>
            </tbody>
          </table>
        </div>
      </div>
    </Show>
  )
}
