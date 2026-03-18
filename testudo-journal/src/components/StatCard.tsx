import { For } from 'solid-js'

export interface StatItem {
  label: string
  value: string
  colorClass?: string
}

interface StatCardProps {
  title: string
  items: StatItem[]
}

export function StatCard(props: StatCardProps) {
  return (
    <div class="bg-elevated border border-container-border rounded-lg p-5">
      <h3 class="font-display text-xs font-semibold tracking-[0.2em] text-text-tertiary uppercase mb-4">
        {props.title}
      </h3>
      <div class="space-y-3">
        <For each={props.items}>
          {(item) => (
            <div class="flex items-center justify-between">
              <span class="font-display text-sm text-text-secondary">{item.label}</span>
              <span class={`font-mono text-sm font-bold ${item.colorClass ?? 'text-text-primary'}`}>
                {item.value}
              </span>
            </div>
          )}
        </For>
      </div>
    </div>
  )
}
