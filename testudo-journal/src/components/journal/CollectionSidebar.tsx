import { createSignal, For, Show } from 'solid-js'
import type { JournalCollection, TreeNode, CollectionFilters } from '../../lib/collections'
import {
  buildTree,
  createCollection,
  updateCollection,
  deleteCollection,
  getCollectionDepth,
} from '../../lib/collections'

export function CollectionSidebar(props: {
  collections: JournalCollection[]
  activeId: string | null
  onSelect: (collection: JournalCollection | null) => void
  onChange: () => void
  collapsed: boolean
  onToggleCollapse: () => void
  currentFilters?: CollectionFilters
}) {
  const [creatingParentId, setCreatingParentId] = createSignal<string | null | undefined>(undefined)
  const [newName, setNewName] = createSignal('')
  const [renamingId, setRenamingId] = createSignal<string | null>(null)
  const [renameValue, setRenameValue] = createSignal('')

  const tree = () => buildTree(props.collections)

  function handleCreate(parentId: string | null) {
    setCreatingParentId(parentId)
    setNewName('')
  }

  function submitCreate() {
    const name = newName().trim()
    if (!name) return
    createCollection(name, {}, creatingParentId() ?? null)
    setCreatingParentId(undefined)
    setNewName('')
    props.onChange()
  }

  function startRename(c: JournalCollection) {
    setRenamingId(c.id)
    setRenameValue(c.name)
  }

  function submitRename() {
    const id = renamingId()
    const name = renameValue().trim()
    if (!id || !name) return
    updateCollection(id, { name })
    setRenamingId(null)
    props.onChange()
  }

  function handleDelete(id: string) {
    deleteCollection(id)
    if (props.activeId === id) props.onSelect(null)
    props.onChange()
  }

  function handleSaveAsCurrent() {
    const filters = props.currentFilters
    if (!filters) return
    const parts: string[] = []
    if (filters.entry_type) parts.push(filters.entry_type)
    if (filters.tag_name) parts.push(filters.tag_name)
    if (filters.date_from) parts.push(`from ${filters.date_from}`)
    const name = parts.length > 0 ? parts.join(' + ') : 'New Collection'
    createCollection(name, filters)
    props.onChange()
  }

  const hasActiveFilters = () => {
    const f = props.currentFilters
    return f && (f.entry_type || f.tag_name || f.date_from || f.date_to)
  }

  return (
    <Show
      when={!props.collapsed}
      fallback={
        <button
          class="flex-shrink-0 w-8 border-r border-container-border flex items-center justify-center hover:bg-container-bg transition-colors"
          onClick={props.onToggleCollapse}
          title="Show sidebar"
        >
          <span class="font-mono text-xs text-text-tertiary rotate-90 whitespace-nowrap">Collections</span>
        </button>
      }
    >
      <nav class="w-56 flex-shrink-0 border-r border-container-border flex flex-col overflow-hidden">
        {/* Header */}
        <div class="flex items-center justify-between px-3 py-2 border-b border-container-border">
          <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">Collections</span>
          <button
            class="font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
            onClick={props.onToggleCollapse}
            title="Collapse sidebar"
          >
            [&lt;]
          </button>
        </div>

        {/* Tree */}
        <div class="flex-1 overflow-y-auto py-1">
          {/* All Entries — root */}
          <button
            class="w-full text-left px-3 py-1.5 font-mono text-xs transition-colors"
            classList={{
              'bg-container-bg text-text-primary': props.activeId === null,
              'text-text-secondary hover:text-text-primary hover:bg-container-bg/50': props.activeId !== null,
            }}
            onClick={() => props.onSelect(null)}
          >
            All Entries
          </button>

          <For each={tree()}>
            {(node) => (
              <CollectionNode
                node={node}
                depth={0}
                activeId={props.activeId}
                renamingId={renamingId()}
                renameValue={renameValue()}
                collections={props.collections}
                onSelect={props.onSelect}
                onStartRename={startRename}
                onRenameInput={setRenameValue}
                onSubmitRename={submitRename}
                onCancelRename={() => setRenamingId(null)}
                onDelete={handleDelete}
                onCreateChild={handleCreate}
              />
            )}
          </For>

          {/* Inline create form */}
          <Show when={creatingParentId() !== undefined}>
            <div class="px-3 py-1" style={{ 'padding-left': `${((creatingParentId() ? 1 : 0) + 1) * 12}px` }}>
              <input
                class="w-full bg-container-bg border border-container-border px-2 py-1 font-mono text-xs text-text-primary placeholder:text-text-tertiary"
                placeholder="Collection name..."
                value={newName()}
                onInput={(e) => setNewName(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') submitCreate()
                  if (e.key === 'Escape') setCreatingParentId(undefined)
                }}
                autofocus
              />
            </div>
          </Show>
        </div>

        {/* Footer actions */}
        <div class="border-t border-container-border px-3 py-2 space-y-1">
          <Show when={hasActiveFilters()}>
            <button
              class="w-full text-left font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
              onClick={handleSaveAsCurrent}
            >
              + Save Current Filters
            </button>
          </Show>
          <button
            class="w-full text-left font-mono text-xs text-text-tertiary hover:text-text-primary transition-colors"
            onClick={() => handleCreate(null)}
          >
            + New Collection
          </button>
        </div>
      </nav>
    </Show>
  )
}

// --- Tree node (recursive) ---

function CollectionNode(props: {
  node: TreeNode
  depth: number
  activeId: string | null
  renamingId: string | null
  renameValue: string
  collections: JournalCollection[]
  onSelect: (c: JournalCollection) => void
  onStartRename: (c: JournalCollection) => void
  onRenameInput: (v: string) => void
  onSubmitRename: () => void
  onCancelRename: () => void
  onDelete: (id: string) => void
  onCreateChild: (parentId: string) => void
}) {
  const [expanded, setExpanded] = createSignal(true)
  const [showActions, setShowActions] = createSignal(false)

  const c = () => props.node.collection
  const hasChildren = () => props.node.children.length > 0
  const isActive = () => props.activeId === c().id
  const isRenaming = () => props.renamingId === c().id
  const canNest = () => getCollectionDepth(props.collections, c().id) < 2

  const paddingLeft = () => `${(props.depth + 1) * 12}px`

  return (
    <div>
      <Show
        when={!isRenaming()}
        fallback={
          <div class="px-3 py-1" style={{ 'padding-left': paddingLeft() }}>
            <input
              class="w-full bg-container-bg border border-container-border px-2 py-1 font-mono text-xs text-text-primary"
              value={props.renameValue}
              onInput={(e) => props.onRenameInput(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') props.onSubmitRename()
                if (e.key === 'Escape') props.onCancelRename()
              }}
              autofocus
            />
          </div>
        }
      >
        <div
          class="group flex items-center gap-1 pr-2 py-1 cursor-pointer transition-colors"
          classList={{
            'bg-container-bg text-text-primary': isActive(),
            'text-text-secondary hover:text-text-primary hover:bg-container-bg/50': !isActive(),
          }}
          style={{ 'padding-left': paddingLeft() }}
          onClick={() => props.onSelect(c())}
          onMouseEnter={() => setShowActions(true)}
          onMouseLeave={() => setShowActions(false)}
        >
          {/* Expand/collapse toggle */}
          <Show when={hasChildren()}>
            <button
              class="w-4 h-4 flex items-center justify-center font-mono text-[10px] text-text-tertiary hover:text-text-primary flex-shrink-0"
              onClick={(e) => { e.stopPropagation(); setExpanded(!expanded()) }}
            >
              {expanded() ? '▼' : '▶'}
            </button>
          </Show>
          <Show when={!hasChildren()}>
            <span class="w-4 flex-shrink-0" />
          </Show>

          {/* Name */}
          <span class="font-mono text-xs truncate flex-1">{c().name}</span>

          {/* Hover actions */}
          <Show when={showActions()}>
            <div class="flex items-center gap-0.5 flex-shrink-0">
              <Show when={canNest()}>
                <button
                  class="font-mono text-[10px] text-text-tertiary hover:text-text-primary transition-colors px-0.5"
                  onClick={(e) => { e.stopPropagation(); props.onCreateChild(c().id) }}
                  title="Add sub-collection"
                >
                  +
                </button>
              </Show>
              <button
                class="font-mono text-[10px] text-text-tertiary hover:text-text-primary transition-colors px-0.5"
                onClick={(e) => { e.stopPropagation(); props.onStartRename(c()) }}
                title="Rename"
              >
                ✎
              </button>
              <button
                class="font-mono text-[10px] text-text-tertiary hover:text-signal-red transition-colors px-0.5"
                onClick={(e) => { e.stopPropagation(); props.onDelete(c().id) }}
                title="Delete"
              >
                ×
              </button>
            </div>
          </Show>
        </div>
      </Show>

      {/* Children */}
      <Show when={expanded()}>
        <For each={props.node.children}>
          {(child) => (
            <CollectionNode
              node={child}
              depth={props.depth + 1}
              activeId={props.activeId}
              renamingId={props.renamingId}
              renameValue={props.renameValue}
              collections={props.collections}
              onSelect={props.onSelect}
              onStartRename={props.onStartRename}
              onRenameInput={props.onRenameInput}
              onSubmitRename={props.onSubmitRename}
              onCancelRename={props.onCancelRename}
              onDelete={props.onDelete}
              onCreateChild={props.onCreateChild}
            />
          )}
        </For>
      </Show>
    </div>
  )
}
