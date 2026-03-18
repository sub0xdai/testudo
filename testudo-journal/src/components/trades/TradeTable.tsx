import { createSignal, createResource, Show, For } from 'solid-js'
import { fetchTrades, type TradeListParams } from '../../api/client'
import { useFilters } from '../filterContext'
import { TradeRow } from './TradeRow'
import { TradeFilters, type TradeFilterState } from './TradeFilters'
import { Pagination } from './Pagination'

type SortState = { field: string; order: 'asc' | 'desc' }

const COLUMNS = [
  { key: 'closed_at', label: 'DATE', sortable: true, align: 'left' },
  { key: 'symbol', label: 'SYMBOL', sortable: false, align: 'left' },
  { key: 'exchange', label: 'EXCH', sortable: false, align: 'left' },
  { key: 'side', label: 'SIDE', sortable: false, align: 'left' },
  { key: 'entry', label: 'ENTRY', sortable: false, align: 'right' },
  { key: 'exit', label: 'EXIT', sortable: false, align: 'right' },
  { key: 'net_pnl', label: 'NET P&L', sortable: true, align: 'right' },
  { key: 'r_multiple', label: 'R', sortable: true, align: 'right' },
  { key: 'duration_secs', label: 'DURATION', sortable: true, align: 'right' },
  { key: 'tags', label: 'TAGS', sortable: false, align: 'left' },
] as const

export function TradeTable(props: { onSelectTrade: (id: string) => void }) {
  const { filters } = useFilters()
  const [page, setPage] = createSignal(1)
  const [sort, setSort] = createSignal<SortState>({ field: 'closed_at', order: 'desc' })
  const [localFilters, setLocalFilters] = createSignal<TradeFilterState>({})

  const params = (): TradeListParams => ({
    page: page(),
    limit: 50,
    sort: sort().field,
    order: sort().order,
    exchange: localFilters().exchange || filters().exchange,
    symbol: localFilters().symbol || filters().symbol,
    side: localFilters().side,
    tag: localFilters().tag,
    dateFrom: filters().dateFrom,
    dateTo: filters().dateTo,
  })

  const [data, { refetch }] = createResource(params, fetchTrades)

  const totalPages = () => {
    const d = data()
    if (!d) return 1
    return Math.max(1, Math.ceil(d.total / d.limit))
  }

  function toggleSort(field: string) {
    const s = sort()
    if (s.field === field) {
      setSort({ field, order: s.order === 'asc' ? 'desc' : 'asc' })
    } else {
      setSort({ field, order: 'desc' })
    }
    setPage(1)
  }

  function handleFilterChange(f: TradeFilterState) {
    setLocalFilters(f)
    setPage(1)
  }

  return (
    <div>
      <TradeFilters filters={localFilters()} onChange={handleFilterChange} />

      <div class="overflow-x-auto">
        <table class="w-full">
          <thead>
            <tr class="border-b border-container-border">
              <For each={COLUMNS}>
                {(col) => (
                  <th
                    class={`px-3 py-2.5 text-[10px] font-display font-medium tracking-widest uppercase whitespace-nowrap ${
                      col.align === 'right' ? 'text-right' : 'text-left'
                    } ${col.sortable ? 'cursor-pointer hover:text-text-primary' : ''} ${
                      sort().field === col.key ? 'text-signal-green' : 'text-text-tertiary'
                    }`}
                    onClick={() => col.sortable && toggleSort(col.key)}
                  >
                    {col.label}
                    <Show when={col.sortable && sort().field === col.key}>
                      <span class="ml-1">{sort().order === 'asc' ? '▲' : '▼'}</span>
                    </Show>
                  </th>
                )}
              </For>
            </tr>
          </thead>
          <tbody>
            <Show
              when={!data.loading}
              fallback={
                <For each={Array(10)}>
                  {() => (
                    <tr class="border-b border-container-border/30">
                      <For each={COLUMNS}>
                        {() => (
                          <td class="px-3 py-2.5">
                            <div class="h-3 bg-container-border/20 rounded animate-pulse" />
                          </td>
                        )}
                      </For>
                    </tr>
                  )}
                </For>
              }
            >
              <Show
                when={data()?.trades.length}
                fallback={
                  <tr>
                    <td colspan={COLUMNS.length} class="px-3 py-12 text-center text-text-tertiary font-mono text-sm">
                      NO TRADES FOUND
                    </td>
                  </tr>
                }
              >
                <For each={data()?.trades}>
                  {(trade) => (
                    <TradeRow
                      trade={trade}
                      onClick={() => props.onSelectTrade(trade.id)}
                    />
                  )}
                </For>
              </Show>
            </Show>
          </tbody>
        </table>
      </div>

      <Show when={data() && totalPages() > 1}>
        <div class="flex items-center justify-between border-t border-container-border">
          <span class="text-xs font-mono text-text-tertiary px-3">
            {data()!.total} trades
          </span>
          <Pagination page={page()} totalPages={totalPages()} onPageChange={setPage} />
        </div>
      </Show>
    </div>
  )
}
