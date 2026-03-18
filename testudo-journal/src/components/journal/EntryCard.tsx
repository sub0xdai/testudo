import { Show, For } from 'solid-js'
import type { JournalEntry, JournalTag } from '../../api/client'
import { MarkdownPreview } from './MarkdownPreview'
import { TagBadge } from '../trades/TagBadge'
import { exportEntry } from '../../lib/export'

const TYPE_STYLES: Record<string, { color: string; label: string }> = {
  'note': { color: '#94a3b8', label: 'NOTE' },
  'pre-trade': { color: '#f59e0b', label: 'PRE-TRADE' },
  'post-trade': { color: '#22C55E', label: 'POST-TRADE' },
  'daily-review': { color: '#888888', label: 'DAILY' },
  'weekly-review': { color: '#888888', label: 'WEEKLY' },
}

function formatTime(iso: string): string {
  const d = new Date(iso)
  return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit', hour12: true })
}

export function EntryCard(props: {
  entry: JournalEntry
  tags?: JournalTag[]
  tradeLabel?: string
  onEdit: () => void
  onDelete: () => void
}) {
  const typeStyle = () => TYPE_STYLES[props.entry.entry_type] ?? TYPE_STYLES['note']

  return (
    <div class="bg-container-bg border border-container-border rounded-lg overflow-hidden hover:border-container-border/80 transition-colors">
      {/* Header bar with type badge */}
      <div
        class="px-4 py-2 flex items-center gap-3 border-b border-container-border"
        style={{ 'border-left': `3px solid ${typeStyle().color}` }}
      >
        <span
          class="font-mono text-[10px] tracking-[0.15em] font-bold px-2 py-0.5 rounded"
          style={{ color: typeStyle().color, background: `${typeStyle().color}15` }}
        >
          {typeStyle().label}
        </span>
        <h3 class="font-display text-sm font-semibold text-text-primary flex-1 truncate">
          {props.entry.title}
        </h3>
        <Show when={props.tags && props.tags.length > 0}>
          <div class="flex gap-1 flex-shrink-0">
            <For each={props.tags}>
              {(tag, i) => <TagBadge tag={tag} index={i()} />}
            </For>
          </div>
        </Show>
      </div>

      {/* Trade link */}
      <Show when={props.tradeLabel}>
        <div class="px-4 pt-2 font-mono text-xs text-text-tertiary">
          Linked: {props.tradeLabel}
        </div>
      </Show>

      {/* Body preview */}
      <div class="px-4 py-3 max-h-40 overflow-hidden relative">
        <MarkdownPreview content={props.entry.body} />
        <div class="absolute bottom-0 left-0 right-0 h-8 bg-gradient-to-t from-container-bg to-transparent" />
      </div>

      {/* Footer */}
      <div class="px-4 py-2 flex items-center justify-between border-t border-container-border">
        <span class="font-mono text-xs text-text-tertiary">
          {formatTime(props.entry.created_at)}
        </span>
        <div class="flex gap-2">
          <button
            class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
            onClick={() => exportEntry(props.entry, props.tags)}
          >
            [Export]
          </button>
          <button
            class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
            onClick={props.onEdit}
          >
            [Edit]
          </button>
          <button
            class="font-mono text-xs text-text-tertiary hover:text-signal-red transition-colors"
            onClick={props.onDelete}
          >
            [Delete]
          </button>
        </div>
      </div>
    </div>
  )
}
