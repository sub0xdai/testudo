import { Show, type JSX } from 'solid-js'
import { SkeletonBar } from '../SkeletonBar'

interface ChartContainerProps {
  title: string
  loading: boolean
  empty: boolean
  error?: string
  class?: string
  children: JSX.Element
}

export function ChartContainer(props: ChartContainerProps) {
  return (
    <div class={`bg-elevated border border-container-border rounded-lg p-5 ${props.class ?? ''}`}>
      <h3 class="font-display text-xs font-semibold tracking-section text-text-tertiary uppercase mb-4">
        {props.title}
      </h3>

      <Show when={props.loading}>
        <div class="relative h-48">
          {/* Y-axis ticks */}
          <div class="absolute left-0 top-0 bottom-6 w-8 flex flex-col justify-between">
            <SkeletonBar width="30px" height="8px" />
            <SkeletonBar width="24px" height="8px" />
            <SkeletonBar width="28px" height="8px" />
          </div>
          {/* Chart area with grid */}
          <div class="ml-10 h-full border-l border-b border-container-border/20 relative">
            <div class="absolute inset-0 skeleton-shimmer" />
          </div>
        </div>
      </Show>

      <Show when={!props.loading && props.error}>
        <div class="flex items-center justify-center h-48">
          <div class="text-center">
            <span class="font-display text-xs tracking-section text-signal-red uppercase">
              FAILED TO LOAD
            </span>
            <p class="font-mono text-xs text-text-tertiary mt-1">{props.error}</p>
          </div>
        </div>
      </Show>

      <Show when={!props.loading && !props.error && props.empty}>
        <div class="flex items-center justify-center h-48">
          <div class="font-mono text-xs text-text-tertiary">NO DATA</div>
        </div>
      </Show>

      <Show when={!props.loading && !props.error && !props.empty}>
        {props.children}
      </Show>
    </div>
  )
}
