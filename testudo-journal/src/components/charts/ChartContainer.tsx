import { Show, type JSX } from 'solid-js'

interface ChartContainerProps {
  title: string
  loading: boolean
  empty: boolean
  class?: string
  children: JSX.Element
}

export function ChartContainer(props: ChartContainerProps) {
  return (
    <div class={`bg-elevated border border-container-border rounded-lg p-5 ${props.class ?? ''}`}>
      <h3 class="font-display text-xs font-semibold tracking-[0.2em] text-text-tertiary uppercase mb-4">
        {props.title}
      </h3>

      <Show when={props.loading}>
        <div class="flex items-center justify-center h-48">
          <div class="font-mono text-xs text-text-tertiary animate-pulse">LOADING...</div>
        </div>
      </Show>

      <Show when={!props.loading && props.empty}>
        <div class="flex items-center justify-center h-48">
          <div class="font-mono text-xs text-text-tertiary">NO DATA</div>
        </div>
      </Show>

      <Show when={!props.loading && !props.empty}>
        {props.children}
      </Show>
    </div>
  )
}
