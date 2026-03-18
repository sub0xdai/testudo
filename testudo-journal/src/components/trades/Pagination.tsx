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
    <div class="flex items-center justify-center gap-1 py-4 font-mono text-sm">
      <button
        class="px-2 py-1 text-text-secondary hover:text-text-primary disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        disabled={props.page <= 1}
        onClick={() => props.onPageChange(props.page - 1)}
      >
        &larr;
      </button>
      <For each={pages()}>
        {(p) =>
          p === '...' ? (
            <span class="px-2 py-1 text-text-tertiary">&hellip;</span>
          ) : (
            <button
              class={`px-2 py-1 transition-colors ${
                p === props.page
                  ? 'text-signal-green border-b border-signal-green'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
              onClick={() => props.onPageChange(p as number)}
            >
              {p}
            </button>
          )
        }
      </For>
      <button
        class="px-2 py-1 text-text-secondary hover:text-text-primary disabled:opacity-30 disabled:cursor-not-allowed transition-colors"
        disabled={props.page >= props.totalPages}
        onClick={() => props.onPageChange(props.page + 1)}
      >
        &rarr;
      </button>
    </div>
  )
}
