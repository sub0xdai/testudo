import { For } from 'solid-js'

export interface StatItem {
  label: string
  value: string
  colorClass?: string
}

interface StatSectionProps {
  title: string
  items: StatItem[]
}

export function StatSection(props: StatSectionProps) {
  return (
    <div>
      <div class="font-display text-xs font-bold tracking-section text-text-secondary uppercase px-5 py-3 border-b border-container-border/50">
        {props.title}
      </div>
      <div class="py-1">
        <For each={props.items}>
          {(item) => (
            <div class="flex items-center justify-between px-5 py-1.5">
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
