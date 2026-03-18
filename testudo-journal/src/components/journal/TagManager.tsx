import { createSignal, Show, For } from 'solid-js'
import { createTag, updateTag, deleteTag, type JournalTag } from '../../api/client'
import { useEscapeClose } from '../../lib/useEscapeClose'
import { createFocusTrap } from '../../lib/createFocusTrap'
import { TAG_PALETTE, CLOSE_ANIMATION_MS } from '../../lib/tokens'

export function TagManager(props: {
  tags: JournalTag[]
  onUpdate: () => void
  onClose: () => void
}) {
  const [newName, setNewName] = createSignal('')
  const [newColor, setNewColor] = createSignal(TAG_PALETTE[0])
  const [editingId, setEditingId] = createSignal<string | null>(null)
  const [editName, setEditName] = createSignal('')
  const [editColor, setEditColor] = createSignal('')
  const [loading, setLoading] = createSignal(false)
  const [closing, setClosing] = createSignal(false)
  let dialogRef!: HTMLDivElement

  createFocusTrap(() => dialogRef)

  function requestClose() {
    setClosing(true)
    setTimeout(props.onClose, CLOSE_ANIMATION_MS)
  }

  useEscapeClose(requestClose)

  async function handleCreate() {
    const name = newName().trim()
    if (!name) return
    setLoading(true)
    try {
      await createTag({ name, color: newColor() })
      setNewName('')
      props.onUpdate()
    } catch (e) {
      console.error('Failed to create tag:', e)
    }
    setLoading(false)
  }

  async function handleUpdate(tagId: string) {
    const name = editName().trim()
    if (!name) return
    setLoading(true)
    try {
      await updateTag(tagId, { name, color: editColor() })
      setEditingId(null)
      props.onUpdate()
    } catch (e) {
      console.error('Failed to update tag:', e)
    }
    setLoading(false)
  }

  async function handleDelete(tagId: string) {
    setLoading(true)
    try {
      await deleteTag(tagId)
      props.onUpdate()
    } catch (e) {
      console.error('Failed to delete tag:', e)
    }
    setLoading(false)
  }

  function startEdit(tag: JournalTag) {
    setEditingId(tag.id)
    setEditName(tag.name)
    setEditColor(tag.color || TAG_PALETTE[0])
  }

  return (
    <div class="fixed inset-0 z-50 flex items-center justify-center">
      <div class={`absolute inset-0 bg-black/60 ${closing() ? 'animate-fade-out' : 'animate-fade-in'}`} onClick={requestClose} />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="tag-manager-title"
        class={`relative bg-elevated border border-container-border rounded-lg w-full max-w-md p-6 ${closing() ? 'animate-scale-out' : 'animate-scale-in'}`}
      >
        <div class="flex items-center justify-between mb-6">
          <h2 id="tag-manager-title" class="font-display text-sm tracking-section text-text-primary uppercase">Tag Manager</h2>
          <button
            class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
            onClick={requestClose}
            aria-label="Close tag manager"
          >
            [Close]
          </button>
        </div>

        {/* Existing tags */}
        <div class="space-y-2 mb-6 max-h-64 overflow-y-auto">
          <For each={props.tags}>
            {(tag) => (
              <Show
                when={editingId() === tag.id}
                fallback={
                  <div class="flex items-center gap-3 px-3 py-2 rounded border border-container-border">
                    <span class="w-3 h-3 rounded-full flex-shrink-0" style={{ background: tag.color || '#94a3b8' }} />
                    <span class="font-mono text-sm text-text-primary flex-1">{tag.name}</span>
                    <button
                      class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
                      onClick={() => startEdit(tag)}
                    >
                      [Edit]
                    </button>
                    <button
                      class="font-mono text-xs text-text-tertiary hover:text-signal-red transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                      onClick={() => handleDelete(tag.id)}
                      disabled={loading()}
                    >
                      [Del]
                    </button>
                  </div>
                }
              >
                <div class="flex items-center gap-2 px-3 py-2 rounded border border-border-active">
                  <div class="flex gap-1 flex-shrink-0">
                    <For each={TAG_PALETTE}>
                      {(c) => (
                        <button
                          class="w-4 h-4 rounded-full border transition-transform"
                          classList={{ 'scale-125 border-white': editColor() === c, 'border-transparent': editColor() !== c }}
                          style={{ background: c }}
                          onClick={() => setEditColor(c)}
                          aria-label={`Color ${c}`}
                        />
                      )}
                    </For>
                  </div>
                  <input
                    class="flex-1 bg-container-bg border border-container-border rounded px-2 py-1 font-mono text-xs text-text-primary"
                    value={editName()}
                    onInput={(e) => setEditName(e.currentTarget.value)}
                    onKeyDown={(e) => e.key === 'Enter' && handleUpdate(tag.id)}
                  />
                  <button
                    class="font-mono text-xs text-text-primary hover:text-text-primary/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                    onClick={() => handleUpdate(tag.id)}
                    disabled={loading()}
                  >
                    [Save]
                  </button>
                  <button
                    class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
                    onClick={() => setEditingId(null)}
                  >
                    [X]
                  </button>
                </div>
              </Show>
            )}
          </For>
          <Show when={props.tags.length === 0}>
            <div class="font-mono text-xs text-text-tertiary text-center py-4">No tags yet</div>
          </Show>
        </div>

        {/* New tag */}
        <div class="border-t border-container-border pt-4">
          <div class="flex gap-1 mb-3">
            <For each={TAG_PALETTE}>
              {(c) => (
                <button
                  class="w-5 h-5 rounded-full border transition-transform"
                  classList={{ 'scale-125 border-white': newColor() === c, 'border-transparent': newColor() !== c }}
                  style={{ background: c }}
                  onClick={() => setNewColor(c)}
                  aria-label={`Color ${c}`}
                />
              )}
            </For>
          </div>
          <div class="flex gap-2">
            <input
              placeholder="New tag name..."
              class="flex-1 bg-container-bg border border-container-border rounded px-3 py-2 font-mono text-sm text-text-primary placeholder:text-text-tertiary"
              value={newName()}
              onInput={(e) => setNewName(e.currentTarget.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleCreate()}
            />
            <button
              class="px-4 py-2 border border-text-primary text-text-primary font-mono text-xs rounded hover:bg-text-primary hover:text-main-bg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              onClick={handleCreate}
              disabled={loading() || !newName().trim()}
            >
              + New Tag
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
