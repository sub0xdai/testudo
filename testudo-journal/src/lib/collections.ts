/** @anchor infra:journal-lib:collections
 * @tags infra */

// JNL-17: Nested collections — localStorage-backed persistence layer
// Swap localStorage calls for API calls when backend adds /journal/collections

const STORAGE_KEY = 'testudo_journal_collections'

export interface CollectionFilters {
  entry_type?: string
  tag_name?: string
  date_from?: string
  date_to?: string
}

export interface JournalCollection {
  id: string
  parent_id: string | null
  name: string
  icon?: string
  sort_order: number
  filters: CollectionFilters
  sort_by: string
  sort_dir: 'asc' | 'desc'
  created_at: string
  updated_at: string
}

export interface TreeNode {
  collection: JournalCollection
  children: TreeNode[]
}

// --- CRUD ---

function generateId(): string {
  return crypto.randomUUID()
}

function readAll(): JournalCollection[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    return JSON.parse(raw) as JournalCollection[]
  } catch {
    return []
  }
}

function writeAll(collections: JournalCollection[]): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(collections))
}

export function getCollections(): JournalCollection[] {
  return readAll()
}

export function createCollection(
  name: string,
  filters: CollectionFilters = {},
  parentId: string | null = null,
  sortBy = 'created_at',
  sortDir: 'asc' | 'desc' = 'desc',
): JournalCollection {
  const all = readAll()

  // Enforce max 3 levels deep
  if (parentId) {
    const depth = getDepth(all, parentId)
    if (depth >= 2) throw new Error('Maximum nesting depth (3 levels) reached')
  }

  const siblings = all.filter((c) => c.parent_id === parentId)
  const now = new Date().toISOString()
  const collection: JournalCollection = {
    id: generateId(),
    parent_id: parentId,
    name,
    sort_order: siblings.length,
    filters,
    sort_by: sortBy,
    sort_dir: sortDir,
    created_at: now,
    updated_at: now,
  }

  all.push(collection)
  writeAll(all)
  return collection
}

export function updateCollection(
  id: string,
  updates: Partial<Pick<JournalCollection, 'name' | 'icon' | 'filters' | 'sort_by' | 'sort_dir' | 'parent_id'>>,
): JournalCollection | null {
  const all = readAll()
  const idx = all.findIndex((c) => c.id === id)
  if (idx === -1) return null

  // If moving to new parent, enforce depth limit
  if (updates.parent_id !== undefined && updates.parent_id !== all[idx].parent_id) {
    if (updates.parent_id) {
      const depth = getDepth(all, updates.parent_id)
      if (depth >= 2) throw new Error('Maximum nesting depth (3 levels) reached')
    }
  }

  all[idx] = { ...all[idx], ...updates, updated_at: new Date().toISOString() }
  writeAll(all)
  return all[idx]
}

export function deleteCollection(id: string): void {
  const all = readAll()
  // Move children to root (spec: "Deleting a parent moves children to root level")
  for (const c of all) {
    if (c.parent_id === id) c.parent_id = null
  }
  writeAll(all.filter((c) => c.id !== id))
}

export function reorderCollections(ids: string[], parentId: string | null): void {
  const all = readAll()
  for (let i = 0; i < ids.length; i++) {
    const c = all.find((c) => c.id === ids[i])
    if (c && c.parent_id === parentId) c.sort_order = i
  }
  writeAll(all)
}

// --- Tree builder ---

export function buildTree(collections: JournalCollection[]): TreeNode[] {
  const map = new Map<string, TreeNode>()
  const roots: TreeNode[] = []

  const sorted = [...collections].sort((a, b) => a.sort_order - b.sort_order)

  for (const c of sorted) {
    map.set(c.id, { collection: c, children: [] })
  }

  for (const c of sorted) {
    const node = map.get(c.id)!
    if (c.parent_id && map.has(c.parent_id)) {
      map.get(c.parent_id)!.children.push(node)
    } else {
      roots.push(node)
    }
  }

  return roots
}

// --- Helpers ---

function getDepth(collections: JournalCollection[], id: string): number {
  let depth = 0
  let current = collections.find((c) => c.id === id)
  while (current?.parent_id) {
    depth++
    current = collections.find((c) => c.id === current!.parent_id)
  }
  return depth
}

export function getCollectionDepth(collections: JournalCollection[], id: string): number {
  return getDepth(collections, id)
}
