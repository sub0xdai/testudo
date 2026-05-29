/** @anchor ui:journal:Pagination
 * @tags ui */

import { For } from 'solid-js'

export function Pagination(props: {
  page: number
  totalPages: number
  onPageChange: (page: number) => void
}) {
  const pages = () => {
    const total = props.totalPages
    const current = props.page
    const result: (number | '...')[] = []
    if (total <= 7) {
      for (let i = 1; i <= total; i++) result.push(i)
      return result
    }
    result.push(1)
    if (current > 3) result.push('...')
    const start = Math.max(2, current - 1)
    const end = Math.min(total - 1, current + 1)
    for (let i = start; i <= end; i++) result.push(i)
    if (current < total - 2) result.push('...')
    result.push(total)
    return result
  }

  return (
    <nav aria-label="Pagination" class="flex items-center justify-center gap-1 py-4 font-mono text-sm">
      <button
        class="btn-ghost px-3 py-2 min-h-[44px] min-w-[44px] flex items-center justify-center disabled:opacity-30 disabled:cursor-not-allowed"
        disabled={props.page <= 1}
        onClick={() => props.onPageChange(props.page - 1)}
        aria-label="Previous page"
      >
        &larr;
      </button>
      <For each={pages()}>
        {(p) =>
          p === '...' ? (
            <span class="px-3 py-2 min-h-[44px] min-w-[44px] flex items-center justify-center text-text-tertiary">&hellip;</span>
          ) : (
            <button
              class={`btn-ghost px-3 py-2 min-h-[44px] min-w-[44px] flex items-center justify-center transition-colors ${
                p === props.page
                  ? 'text-text-primary border-b border-text-primary'
                  : ''
              }`}
              onClick={() => props.onPageChange(p as number)}
              aria-current={p === props.page ? 'page' : undefined}
              aria-label={`Page ${p}`}
            >
              {p}
            </button>
          )
        }
      </For>
      <button
        class="btn-ghost px-3 py-2 min-h-[44px] min-w-[44px] flex items-center justify-center disabled:opacity-30 disabled:cursor-not-allowed"
        disabled={props.page >= props.totalPages}
        onClick={() => props.onPageChange(props.page + 1)}
        aria-label="Next page"
      >
        &rarr;
      </button>
    </nav>
  )
}
