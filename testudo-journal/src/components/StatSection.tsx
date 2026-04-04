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
      <div class="font-display text-xs font-bold tracking-section text-text-secondary uppercase px-6 py-4 border-b border-container-border/50">
        {props.title}
      </div>
      <div class="py-2">
        <For each={props.items}>
          {(item) => (
            <div class="flex items-center gap-2 px-6 py-2.5">
              <span class="font-display text-xs text-text-secondary">{item.label}</span>
              <span class="flex-1 border-b border-dotted border-container-border/30" />
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
