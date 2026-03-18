import { createSignal, createResource, Show, For } from 'solid-js'
import { fetchTrades, type TradeListParams } from '../../api/client'
import { useFilters } from '../filterContext'
import { TradeRow } from './TradeRow'
import { TradeFilters, type TradeFilterState } from './TradeFilters'
import { Pagination } from './Pagination'
import { SkeletonBar } from '../SkeletonBar'

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
                    } ${
                      sort().field === col.key ? 'text-signal-green' : 'text-text-tertiary'
                    }`}
                    aria-sort={col.sortable ? (sort().field === col.key ? (sort().order === 'asc' ? 'ascending' : 'descending') : 'none') : undefined}
                  >
                    {col.sortable ? (
                      <button
                        class={`w-full ${col.align === 'right' ? 'text-right' : 'text-left'} cursor-pointer hover:text-text-primary`}
                        onClick={() => toggleSort(col.key)}
                      >
                        {col.label}
                        <Show when={sort().field === col.key}>
                          <span class="ml-1">{sort().order === 'asc' ? '▲' : '▼'}</span>
                        </Show>
                      </button>
                    ) : col.label}
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
                  {(_, i) => (
                    <tr class="border-b border-container-border/30">
                      <td class="px-3 py-2.5"><SkeletonBar width="72px" /></td>
                      <td class="px-3 py-2.5"><SkeletonBar width="64px" /></td>
                      <td class="px-3 py-2.5"><SkeletonBar width="36px" /></td>
                      <td class="px-3 py-2.5"><SkeletonBar width="32px" /></td>
                      <td class="px-3 py-2.5 text-right"><SkeletonBar width="56px" class="ml-auto" /></td>
                      <td class="px-3 py-2.5 text-right"><SkeletonBar width="56px" class="ml-auto" /></td>
                      <td class="px-3 py-2.5 text-right"><SkeletonBar width="64px" class="ml-auto" /></td>
                      <td class="px-3 py-2.5 text-right"><SkeletonBar width="28px" class="ml-auto" /></td>
                      <td class="px-3 py-2.5 text-right"><SkeletonBar width="44px" class="ml-auto" /></td>
                      <td class="px-3 py-2.5"><SkeletonBar width={i() % 2 === 0 ? '48px' : '32px'} /></td>
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
