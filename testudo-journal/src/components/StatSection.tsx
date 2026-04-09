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
    <div class="border-b border-container-border/50">
      <div class="font-display text-xs font-bold tracking-section text-text-secondary uppercase px-8 py-5">
        {props.title}
      </div>
      <div class="pb-4">
        <For each={props.items}>
          {(item) => (
            <div class="flex items-center gap-2 px-8 py-3">
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
