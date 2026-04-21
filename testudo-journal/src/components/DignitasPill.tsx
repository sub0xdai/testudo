import { createSignal, Show, createResource, onMount, onCleanup } from 'solid-js'
import { useAuth } from '../context/AuthContext'
import { fetchDignitasMe, type DignitasCurrent } from '../api/client'

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

  const [data] = createResource(
    () => auth.isAuthenticated() || undefined,
    () => fetchDignitasMe(),
  )

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
          {/* DignitasPanel — expanded in T10 */}
          <div class="absolute right-0 mt-1 w-72 bg-container-bg border border-container-border z-50 p-4 text-center">
            <p class="font-mono text-[10px] text-text-tertiary tracking-widest">// DIGNITAS_PANEL</p>
          </div>
        </Show>
      </div>
    </Show>
  )
}
