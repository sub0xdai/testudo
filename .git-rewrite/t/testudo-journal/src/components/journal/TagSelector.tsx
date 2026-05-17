import { createSignal, Show, For } from 'solid-js'
import type { JournalTag } from '../../api/client'
import { TagBadge } from '../trades/TagBadge'
import { useEscapeClose } from '../../lib/useEscapeClose'

export function TagSelector(props: {
  allTags: JournalTag[]
  selected: JournalTag[]
  onAdd: (tag: JournalTag) => void
  onRemove: (tag: JournalTag) => void
}) {
  const [open, setOpen] = createSignal(false)
  useEscapeClose(() => setOpen(false))

  const available = () => {
    const selectedIds = new Set(props.selected.map((t) => t.id))
    return props.allTags.filter((t) => !selectedIds.has(t.id))
  }

  return (
    <div class="relative">
      <div class="flex flex-wrap gap-1.5 items-center">
        <For each={props.selected}>
          {(tag, i) => (
            <TagBadge tag={tag} index={i()} onRemove={() => props.onRemove(tag)} />
          )}
        </For>
        <button
          class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors px-2 py-0.5 border border-dashed border-container-border hover:border-text-primary"
          onClick={() => setOpen(!open())}
          aria-haspopup="listbox"
          aria-expanded={open()}
          aria-controls="tag-listbox"
        >
          + Add Tag
        </button>
      </div>

      <Show when={open()}>
        <div
          id="tag-listbox"
          role="listbox"
          aria-label="Available tags"
          class="absolute z-50 top-full left-0 mt-1 bg-elevated border border-container-border shadow-lg shadow-black/30 min-w-48 animate-dropdown-in"
        >
          <Show when={available().length === 0}>
            <div class="px-3 py-2 font-mono text-xs text-text-tertiary">No more tags</div>
          </Show>
          <For each={available()}>
            {(tag) => (
              <button
                role="option"
                class="w-full text-left px-3 py-2 hover:bg-container-bg-hover transition-colors flex items-center gap-2"
                onClick={() => { props.onAdd(tag); setOpen(false) }}
              >
                <span class="w-2 h-2 rounded-full" style={{ background: tag.color || '#94a3b8' }} />
                <span class="font-mono text-xs text-text-primary">{tag.name}</span>
              </button>
            )}
          </For>
        </div>
        <div class="fixed inset-0 z-40" onClick={() => setOpen(false)} />
      </Show>
    </div>
  )
}
