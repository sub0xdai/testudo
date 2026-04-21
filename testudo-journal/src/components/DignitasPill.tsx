import { createSignal, Show, createResource, onMount, onCleanup } from 'solid-js'
import { useAuth } from '../context/AuthContext'
import { fetchDignitasMe, type DignitasCurrent } from '../api/client'
import { DignitasPanel } from './DignitasPanel'

function scoreText(d: DignitasCurrent): string {
  return parseFloat(d.score).toFixed(1)
}

function deltaText(d: DignitasCurrent): string {
  if (d.cold_start || d.delta_7d === null) return '—'
  const n = parseFloat(d.delta_7d)
  if (n > 0) return `▲${Math.abs(n).toFixed(1)}`
  if (n < 0) return `▼${Math.abs(n).toFixed(1)}`
  return '—'
}

function deltaClass(d: DignitasCurrent): string {
  if (d.cold_start || d.delta_7d === null) return 'text-text-tertiary'
  const n = parseFloat(d.delta_7d)
  if (n > 0) return 'text-signal-green'
  if (n < 0) return 'text-signal-red'
  return 'text-text-tertiary'
}

export function DignitasPill() {
  const auth = useAuth()
  const [open, setOpen] = createSignal(false)
  let ref: HTMLDivElement | undefined

  const [data, { refetch }] = createResource(
    () => auth.isAuthenticated() || undefined,
    () => fetchDignitasMe(),
  )

  async function handleHide() {
    await refetch()
    setOpen(false)
  }

  function handleClickOutside(e: MouseEvent) {
    if (ref && !ref.contains(e.target as Node)) setOpen(false)
  }

  onMount(() => document.addEventListener('mousedown', handleClickOutside))
  onCleanup(() => document.removeEventListener('mousedown', handleClickOutside))

  return (
    <Show when={auth.isAuthenticated() && data() && !data()!.pill_hidden}>
      <div ref={ref} class="relative">
        <button
          onClick={() => setOpen(!open())}
          class={`font-mono text-[10px] tracking-wider px-3 py-1.5 border border-container-border hover:border-text-primary transition-colors ${deltaClass(data()!)}`}
          title="Dignitas Score — discipline adherence index"
        >
          DIGNITAS {scoreText(data()!)} {deltaText(data()!)}
        </button>

        <Show when={open()}>
          <div class="absolute right-0 mt-1 w-72 bg-container-bg border border-container-border z-50">
            <DignitasPanel
              data={data()!}
              onHide={handleHide}
              onClose={() => setOpen(false)}
            />
          </div>
        </Show>
      </div>
    </Show>
  )
}
