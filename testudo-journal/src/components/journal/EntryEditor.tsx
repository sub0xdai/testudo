import { createSignal, createResource, Show } from 'solid-js'
import {
  createEntry,
  updateEntry,
  fetchTags,
  addTradeTags,
  removeTradeTag,
  type JournalEntry,
  type JournalTag,
  type JournalTrade,
} from '../../api/client'
import { MarkdownPreview } from './MarkdownPreview'
import { TradeSelector } from './TradeSelector'
import { TagSelector } from './TagSelector'

const ENTRY_TYPES = [
  { value: 'note', label: 'Note' },
  { value: 'pre-trade', label: 'Pre-Trade' },
  { value: 'post-trade', label: 'Post-Trade' },
  { value: 'daily-review', label: 'Daily Review' },
  { value: 'weekly-review', label: 'Weekly Review' },
]

export function EntryEditor(props: {
  entry?: JournalEntry
  linkedTrade?: JournalTrade
  linkedTags?: JournalTag[]
  onSave: () => void
  onClose: () => void
}) {
  const isEdit = () => !!props.entry

  const [title, setTitle] = createSignal(props.entry?.title ?? '')
  const [body, setBody] = createSignal(props.entry?.body ?? '')
  const [entryType, setEntryType] = createSignal(props.entry?.entry_type ?? 'note')
  const [linkedTrade, setLinkedTrade] = createSignal<JournalTrade | null>(props.linkedTrade ?? null)
  const [entryDate, setEntryDate] = createSignal(props.entry?.entry_date ?? '')
  const [selectedTags, setSelectedTags] = createSignal<JournalTag[]>(props.linkedTags ?? [])
  const [showPreview, setShowPreview] = createSignal(false)
  const [saving, setSaving] = createSignal(false)
  const [error, setError] = createSignal('')

  const [allTags] = createResource(fetchTags)

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

        // If linked to a trade and tags selected, sync tags
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
    // If editing and linked to a trade, remove the tag from the trade
    if (isEdit() && props.entry?.trade_id) {
      removeTradeTag(props.entry!.trade_id, tag.id).catch(console.error)
    }
  }

  const showDatePicker = () => {
    const t = entryType()
    return t === 'daily-review' || t === 'weekly-review'
  }

  return (
    <div class="fixed inset-0 z-50 flex items-center justify-center">
      <div class="absolute inset-0 bg-black/60" onClick={props.onClose} />
      <div class="relative bg-elevated border border-container-border rounded-lg w-full max-w-3xl max-h-[90vh] flex flex-col">
        {/* Header */}
        <div class="flex items-center justify-between px-6 py-4 border-b border-container-border flex-shrink-0">
          <h2 class="font-display text-sm tracking-[0.2em] text-text-primary uppercase">
            {isEdit() ? 'Edit Entry' : 'New Entry'}
          </h2>
          <div class="flex gap-3">
            <button
              class="px-4 py-2 bg-signal-green/10 border border-signal-green text-signal-green font-mono text-xs rounded hover:bg-signal-green/20 transition-colors disabled:opacity-50"
              onClick={handleSave}
              disabled={saving()}
            >
              {saving() ? 'Saving...' : 'Save'}
            </button>
            <button
              class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
              onClick={props.onClose}
            >
              [Close]
            </button>
          </div>
        </div>

        {/* Form body */}
        <div class="flex-1 overflow-y-auto px-6 py-4 space-y-4">
          <Show when={error()}>
            <div class="font-mono text-xs text-signal-red bg-signal-red/10 border border-signal-red/30 rounded px-3 py-2">
              {error()}
            </div>
          </Show>

          {/* Type selector */}
          <div class="flex gap-2 items-center">
            <label class="font-display text-xs text-text-secondary uppercase tracking-wider w-16">Type</label>
            <select
              class="bg-container-bg border border-container-border rounded px-3 py-2 font-mono text-sm text-text-primary focus:border-border-active focus:outline-none"
              value={entryType()}
              onChange={(e) => setEntryType(e.currentTarget.value)}
            >
              {ENTRY_TYPES.map((t) => (
                <option value={t.value}>{t.label}</option>
              ))}
            </select>
          </div>

          {/* Title */}
          <div class="flex gap-2 items-center">
            <label class="font-display text-xs text-text-secondary uppercase tracking-wider w-16">Title</label>
            <input
              class="flex-1 bg-container-bg border border-container-border rounded px-3 py-2 font-mono text-sm text-text-primary placeholder:text-text-tertiary focus:border-border-active focus:outline-none"
              placeholder="Entry title..."
              value={title()}
              onInput={(e) => setTitle(e.currentTarget.value)}
            />
          </div>

          {/* Trade link (only for new entries or entries already linked) */}
          <Show when={!isEdit()}>
            <div class="flex gap-2 items-start">
              <label class="font-display text-xs text-text-secondary uppercase tracking-wider w-16 pt-2">Trade</label>
              <div class="flex-1">
                <TradeSelector value={linkedTrade()} onSelect={setLinkedTrade} />
              </div>
            </div>
          </Show>

          {/* Date picker for daily/weekly */}
          <Show when={showDatePicker()}>
            <div class="flex gap-2 items-center">
              <label class="font-display text-xs text-text-secondary uppercase tracking-wider w-16">Date</label>
              <input
                type="date"
                class="bg-container-bg border border-container-border rounded px-3 py-2 font-mono text-sm text-text-primary focus:border-border-active focus:outline-none"
                value={entryDate()}
                onInput={(e) => setEntryDate(e.currentTarget.value)}
              />
            </div>
          </Show>

          {/* Tags */}
          <div class="flex gap-2 items-start">
            <label class="font-display text-xs text-text-secondary uppercase tracking-wider w-16 pt-1">Tags</label>
            <div class="flex-1">
              <TagSelector
                allTags={allTags() ?? []}
                selected={selectedTags()}
                onAdd={handleTagAdd}
                onRemove={handleTagRemove}
              />
            </div>
          </div>

          {/* Markdown editor with preview toggle */}
          <div>
            <div class="flex items-center gap-4 mb-2">
              <button
                class="font-mono text-xs transition-colors"
                classList={{
                  'text-signal-green': !showPreview(),
                  'text-text-tertiary hover:text-text-primary': showPreview(),
                }}
                onClick={() => setShowPreview(false)}
              >
                EDIT
              </button>
              <button
                class="font-mono text-xs transition-colors"
                classList={{
                  'text-signal-green': showPreview(),
                  'text-text-tertiary hover:text-text-primary': !showPreview(),
                }}
                onClick={() => setShowPreview(true)}
              >
                PREVIEW
              </button>
            </div>

            <Show
              when={!showPreview()}
              fallback={
                <div class="bg-container-bg border border-container-border rounded p-4 min-h-[300px]">
                  <MarkdownPreview content={body()} />
                </div>
              }
            >
              <textarea
                class="w-full bg-container-bg border border-container-border rounded px-4 py-3 font-mono text-sm text-text-primary placeholder:text-text-tertiary focus:border-border-active focus:outline-none resize-none min-h-[300px]"
                placeholder="Write your journal entry in markdown..."
                value={body()}
                onInput={(e) => setBody(e.currentTarget.value)}
              />
            </Show>
          </div>
        </div>
      </div>
    </div>
  )
}
