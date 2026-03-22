import { createSignal, createResource, Show, onCleanup, onMount } from 'solid-js'
import {
  createEntry,
  updateEntry,
  fetchTags,
  addTradeTags,
  removeTradeTag,
  uploadJournalImage,
  UploadError,
  type JournalEntry,
  type JournalTag,
  type JournalTrade,
} from '../../api/client'
import { MarkdownPreview } from './MarkdownPreview'
import { TradeSelector } from './TradeSelector'
import { TagSelector } from './TagSelector'
import { exportEntry } from '../../lib/export'
import { createFocusTrap } from '../../lib/createFocusTrap'
import { getEntryTypeColors, CLOSE_ANIMATION_MS } from '../../lib/tokens'

function getEntryTypes() {
  const colors = getEntryTypeColors()
  return [
    { value: 'note', label: 'Note', color: colors['note'] },
    { value: 'pre-trade', label: 'Pre-Trade', color: colors['pre-trade'] },
    { value: 'post-trade', label: 'Post-Trade', color: colors['post-trade'] },
    { value: 'daily-review', label: 'Daily', color: colors['daily-review'] },
    { value: 'weekly-review', label: 'Weekly', color: colors['weekly-review'] },
  ]
}

export function EntryEditor(props: {
  entry?: JournalEntry
  linkedTrade?: JournalTrade
  linkedTags?: JournalTag[]
  onSave: () => void
  onClose: () => void
  onStorageChange?: () => void
}) {
  const isEdit = () => !!props.entry
  let textareaRef!: HTMLTextAreaElement
  let dialogRef!: HTMLDivElement

  createFocusTrap(() => dialogRef)

  const [title, setTitle] = createSignal(props.entry?.title ?? '')
  const [body, setBody] = createSignal(props.entry?.body ?? '')
  const [entryType, setEntryType] = createSignal(props.entry?.entry_type ?? 'note')
  const [linkedTrade, setLinkedTrade] = createSignal<JournalTrade | null>(props.linkedTrade ?? null)
  const [entryDate, setEntryDate] = createSignal(props.entry?.entry_date ?? '')
  const [selectedTags, setSelectedTags] = createSignal<JournalTag[]>(props.linkedTags ?? [])
  const [showPreview, setShowPreview] = createSignal(false)
  const [saving, setSaving] = createSignal(false)
  const [uploading, setUploading] = createSignal(false)
  const [error, setError] = createSignal('')
  const [closing, setClosing] = createSignal(false)
  const [dragging, setDragging] = createSignal(false)

  function requestClose() {
    setClosing(true)
    setTimeout(props.onClose, CLOSE_ANIMATION_MS)
  }

  const [allTags] = createResource(fetchTags)

  const typeColor = () => {
    const types = getEntryTypes()
    return types.find((t) => t.value === entryType())?.color ?? '#94a3b8'
  }

  // Keyboard shortcuts
  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') requestClose()
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      handleSave()
    }
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('keydown', handleKeyDown)
    onCleanup(() => window.removeEventListener('keydown', handleKeyDown))
  }

  async function handleSave() {
    const t = title().trim()
    const b = body().trim()
    if (!t || !b) {
      setError('Title and body are required')
      return
    }

    setSaving(true)
    setError('')
    try {
      if (isEdit()) {
        await updateEntry(props.entry!.id, {
          title: t,
          body: b,
          entry_type: entryType(),
        })
      } else {
        const trade = linkedTrade()
        await createEntry({
          title: t,
          body: b,
          entry_type: entryType(),
          trade_id: trade?.id,
          entry_date: entryDate() || undefined,
        })

        if (trade && selectedTags().length > 0) {
          const tagIds = selectedTags().map((t) => t.id)
          await addTradeTags(trade.id, tagIds)
        }
      }
      props.onSave()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save')
    }
    setSaving(false)
  }

  function handleTagAdd(tag: JournalTag) {
    setSelectedTags((prev) => [...prev, tag])
  }

  function handleTagRemove(tag: JournalTag) {
    setSelectedTags((prev) => prev.filter((t) => t.id !== tag.id))
    if (isEdit() && props.entry?.trade_id) {
      removeTradeTag(props.entry!.trade_id, tag.id).catch(console.error)
    }
  }

  // Image upload + insert at cursor
  async function uploadAndInsert(file: File) {
    if (file.size > 5 * 1024 * 1024) {
      setError('File exceeds 5MB limit')
      return
    }
    if (!file.type.startsWith('image/')) {
      setError('Only image files are accepted')
      return
    }
    setUploading(true)
    setError('')
    try {
      const { url } = await uploadJournalImage(file)
      const insertion = `![screenshot](${url})\n`
      const ta = textareaRef
      if (ta) {
        const start = ta.selectionStart
        const before = body().slice(0, start)
        const after = body().slice(ta.selectionEnd)
        setBody(before + insertion + after)
        requestAnimationFrame(() => {
          ta.selectionStart = ta.selectionEnd = start + insertion.length
          ta.focus()
        })
      } else {
        setBody((prev) => prev + insertion)
      }
      props.onStorageChange?.()
    } catch (e) {
      if (e instanceof UploadError && e.code === 'quota_exceeded') {
        setError(e.message)
      } else {
        setError(e instanceof Error ? e.message : 'Upload failed')
      }
    }
    setUploading(false)
  }

  function handlePaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items
    if (!items) return
    for (const item of items) {
      if (item.type.startsWith('image/')) {
        e.preventDefault()
        const file = item.getAsFile()
        if (file) uploadAndInsert(file)
        return
      }
    }
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault()
    setDragging(false)
    const file = e.dataTransfer?.files[0]
    if (file?.type.startsWith('image/')) {
      uploadAndInsert(file)
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault()
    setDragging(true)
  }

  function handleDragLeave() {
    setDragging(false)
  }

  async function handleExport() {
    if (props.entry) {
      await exportEntry(props.entry, selectedTags())
    } else {
      // Export current draft
      const draft: JournalEntry = {
        id: '',
        user_id: '',
        trade_id: null,
        entry_date: entryDate() || null,
        title: title() || 'untitled',
        body: body(),
        entry_type: entryType(),
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      }
      await exportEntry(draft, selectedTags())
    }
  }

  const showDatePicker = () => {
    const t = entryType()
    return t === 'daily-review' || t === 'weekly-review'
  }

  return (
    <div class="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div class={`absolute inset-0 bg-black/80 ${closing() ? 'animate-fade-out' : 'animate-fade-in'}`} onClick={requestClose} />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="entry-editor-title"
        class={`relative bg-elevated border border-container-border w-full max-w-4xl max-h-[95vh] flex flex-col ${closing() ? 'animate-scale-out' : 'animate-scale-in'}`}
      >
        {/* Compact metadata strip */}
        <div class="flex flex-wrap items-center gap-3 px-5 py-3 border-b border-container-border flex-shrink-0">
          <label class="flex items-center gap-1.5">
            <span class="sr-only">Entry type</span>
            <select
              class="bg-container-bg border border-container-border rounded px-2 py-1 font-mono text-xs text-text-primary"
              value={entryType()}
              onChange={(e) => setEntryType(e.currentTarget.value)}
            >
              {getEntryTypes().map((t) => (
                <option value={t.value}>{t.label}</option>
              ))}
            </select>
          </label>

          <input
            id="entry-editor-title"
            class="flex-1 min-w-[200px] bg-transparent border-none font-display text-sm font-semibold text-text-primary placeholder:text-text-tertiary focus:outline-none"
            placeholder="Entry title..."
            value={title()}
            onInput={(e) => setTitle(e.currentTarget.value)}
          />

          <Show when={!isEdit()}>
            <div class="max-w-[200px]">
              <TradeSelector value={linkedTrade()} onSelect={setLinkedTrade} />
            </div>
          </Show>

          <Show when={showDatePicker()}>
            <label class="flex items-center gap-1.5">
              <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">Date</span>
              <input
                type="date"
                class="bg-container-bg border border-container-border rounded px-2 py-1 font-mono text-xs text-text-primary"
                value={entryDate()}
                onInput={(e) => setEntryDate(e.currentTarget.value)}
              />
            </label>
          </Show>

          <div class="flex items-center gap-2 ml-auto flex-shrink-0">
            <button
              class="px-3 py-1.5 border border-text-primary text-text-primary font-mono text-xs rounded hover:bg-text-primary hover:text-main-bg transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              onClick={handleSave}
              disabled={saving()}
            >
              {saving() ? 'Saving...' : 'Save'}
            </button>
            <button
              class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
              onClick={requestClose}
              aria-label="Close editor"
            >
              &times;
            </button>
          </div>
        </div>

        {/* Tags row */}
        <div class="px-5 py-2 border-b border-container-border flex-shrink-0">
          <TagSelector
            allTags={allTags() ?? []}
            selected={selectedTags()}
            onAdd={handleTagAdd}
            onRemove={handleTagRemove}
          />
        </div>

        {/* Error */}
        <Show when={error()}>
          <div role="alert" aria-live="polite" class="mx-5 mt-3 font-mono text-xs text-signal-red bg-signal-red/10 border border-signal-red/30 rounded px-3 py-2">
            {error()}
          </div>
        </Show>

        {/* Tab bar */}
        <div class="flex items-center gap-4 px-5 pt-3 pb-2 flex-shrink-0">
          <button
            class="font-mono text-xs transition-colors"
            classList={{
              'text-text-primary': !showPreview(),
              'text-text-tertiary hover:text-text-primary': showPreview(),
            }}
            onClick={() => setShowPreview(false)}
          >
            EDIT
          </button>
          <button
            class="font-mono text-xs transition-colors"
            classList={{
              'text-text-primary': showPreview(),
              'text-text-tertiary hover:text-text-primary': !showPreview(),
            }}
            onClick={() => setShowPreview(true)}
          >
            PREVIEW
          </button>

          <div class="ml-auto flex gap-2">
            <Show when={uploading()}>
              <span class="font-mono text-xs text-text-tertiary">Uploading...</span>
            </Show>
            <label class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors cursor-pointer">
              Attach
              <input
                type="file"
                accept="image/*"
                class="hidden"
                onChange={(e) => {
                  const file = e.currentTarget.files?.[0]
                  if (file) uploadAndInsert(file)
                  e.currentTarget.value = ''
                }}
              />
            </label>
            <button
              class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
              onClick={handleExport}
            >
              Export .md
            </button>
          </div>
        </div>

        {/* Editor / Preview area */}
        <div class="flex-1 overflow-y-auto px-5 pb-4 min-h-0">
          <Show
            when={!showPreview()}
            fallback={
              <div
                class="bg-container-bg border border-container-border rounded p-4 min-h-[400px]"
                style={{ 'border-left': `3px solid ${typeColor()}` }}
              >
                <MarkdownPreview content={body()} />
              </div>
            }
          >
            <textarea
              ref={textareaRef!}
              class={`w-full bg-container-bg border border-container-border rounded px-4 py-3 font-mono text-sm text-text-primary placeholder:text-text-tertiary resize-y min-h-[400px] ${dragging() ? 'border-text-primary border-dashed' : ''}`}
              style={{ 'border-left': `3px solid ${typeColor()}` }}
              placeholder="Write your journal entry in markdown... Paste images or drag files here."
              value={body()}
              onInput={(e) => setBody(e.currentTarget.value)}
              onPaste={handlePaste}
              onDrop={handleDrop}
              onDragOver={handleDragOver}
              onDragLeave={handleDragLeave}
            />
          </Show>
        </div>

        {/* Footer */}
        <div class="flex items-center justify-between px-5 py-2 border-t border-container-border flex-shrink-0">
          <span class="font-mono text-[10px] text-text-tertiary">
            Ctrl+Enter to save
          </span>
          <button
            class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
            onClick={requestClose}
          >
            [Close]
          </button>
        </div>
      </div>
    </div>
  )
}
