import { Show, type JSX } from 'solid-js'
import { SkeletonBar } from '../SkeletonBar'

interface ChartContainerProps {
  title: string
  loading: boolean
  empty: boolean
  error?: string
  class?: string
  hasActiveFilters?: boolean
  onClearFilters?: () => void
  onRetry?: () => void
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
            <Show when={props.onRetry}>
              <button
                class="mt-3 font-mono text-xs text-text-secondary hover:text-text-primary transition-colors border border-container-border px-3 py-1.5 rounded"
                onClick={props.onRetry}
              >
                Retry
              </button>
            </Show>
          </div>
        </div>
      </Show>

      <Show when={!props.loading && !props.error && props.empty}>
        <div class="flex flex-col items-center justify-center h-48 text-center">
          <p class="font-mono text-xs text-text-tertiary mb-1">
            No {props.title.toLowerCase()} data
          </p>
          <Show when={props.hasActiveFilters}>
            <p class="font-mono text-xs text-text-tertiary mb-3">
              Try adjusting your filters
            </p>
            <button
              class="font-mono text-xs text-text-secondary hover:text-text-primary transition-colors"
              onClick={props.onClearFilters}
            >
              Clear filters
            </button>
          </Show>
        </div>
      </Show>

      <Show when={!props.loading && !props.error && !props.empty}>
        {props.children}
      </Show>
    </div>
  )
}
