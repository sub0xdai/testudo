import { For } from 'solid-js'
import type { StatItem } from './StatCard'

interface StatSectionProps {
  title: string
  items: StatItem[]
}

export function StatSection(props: StatSectionProps) {
  return (
    <div>
      <div class="font-display text-xs tracking-[0.2em] text-text-tertiary uppercase px-4 py-3 border-b border-container-border">
        {props.title}
      </div>
      <div class="py-1">
        <For each={props.items}>
          {(item) => (
            <div class="flex items-center justify-between px-4 py-1.5">
              <span class="font-display text-xs text-text-secondary">{item.label}</span>
              <span class={`font-mono text-xs font-bold ${item.colorClass ?? 'text-text-primary'}`}>
                {item.value}
              </span>
            </div>
          )}
        </For>
      </div>
    </div>
  )
}
