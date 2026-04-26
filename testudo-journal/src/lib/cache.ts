// Mutation → invalidation key map (for future CP callers; CP-3 scope is tags only):
//   addTradeTags / removeTradeTag   → 'tags:'
//   createTag / updateTag / deleteTag → 'tags:'
//   createEntry / updateEntry        → 'entries:', 'journal-timeline:'
//   deleteEntry                      → 'entries:', 'journal-timeline:'
//   saveDraftNotes                   → 'draft:{groupId}'
//   claimHandle / patchVisibility    → 'identity:', 'public-profile:'
//   setCoachPreference / markCoachViewed → 'coach-latest:', 'coach-archive:'
//   patchDignitasPreference          → 'dignitas-me:'

import { createEffect, createSignal, untrack } from 'solid-js'

const DEFAULT_STALE_MS = 30_000
const LS_NAMESPACE = 'testudo:cache:'
const MAX_ENTRY_BYTES = 64 * 1024

type CacheEntry<T> = { data: T; updatedAt: number }

// Exported for testing only — do not read/write externally in production code.
export const _memCache = new Map<string, CacheEntry<unknown>>()

/** Deterministic JSON hash — sorts object keys at every level, omits undefined values. */
export function stableHash(obj: unknown): string {
  return JSON.stringify(canonicalize(obj))
}

function canonicalize(val: unknown): unknown {
  if (val === null || typeof val !== 'object') return val
  if (Array.isArray(val)) return val.map(canonicalize)
  const record = val as Record<string, unknown>
  return Object.fromEntries(
    Object.keys(record)
      .filter(k => record[k] !== undefined)
      .sort()
      .map(k => [k, canonicalize(record[k])]),
  )
}

// Extract the cache key portion from a localStorage key: 'testudo:cache:{identity}:{cacheKey}'
function extractCacheKey(lsKey: string): string | null {
  const after = lsKey.slice(LS_NAMESPACE.length)
  const colonIdx = after.indexOf(':')
  if (colonIdx < 0) return null
  return after.slice(colonIdx + 1)
}

function buildLsKey(identity: string, cacheKey: string): string {
  return `${LS_NAMESPACE}${identity}:${cacheKey}`
}

function lsRead(identity: string, cacheKey: string): CacheEntry<unknown> | null {
  try {
    const raw = localStorage.getItem(buildLsKey(identity, cacheKey))
    if (!raw) return null
    return JSON.parse(raw) as CacheEntry<unknown>
  } catch {
    return null
  }
}

function lsWrite(identity: string, cacheKey: string, entry: CacheEntry<unknown>): void {
  const key = buildLsKey(identity, cacheKey)
  const serialized = JSON.stringify(entry)
  if (serialized.length > MAX_ENTRY_BYTES) return

  function tryWrite() { localStorage.setItem(key, serialized) }

  function evictOldest() {
    let oldest: { key: string; updatedAt: number } | null = null
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i)
      if (!k?.startsWith(LS_NAMESPACE)) continue
      try {
        const val = JSON.parse(localStorage.getItem(k) ?? '') as CacheEntry<unknown>
        if (!oldest || val.updatedAt < oldest.updatedAt) oldest = { key: k, updatedAt: val.updatedAt }
      } catch { /* skip malformed */ }
    }
    if (oldest) localStorage.removeItem(oldest.key)
  }

  try {
    tryWrite()
  } catch {
    evictOldest()
    try {
      tryWrite()
    } catch {
      console.warn('[cache] localStorage quota exceeded; persistence disabled for this entry')
    }
  }
}

/** Drop all cache entries (memCache + localStorage) whose key starts with `keyPrefix`. */
export function invalidate(keyPrefix: string): void {
  for (const k of [..._memCache.keys()]) {
    if (k.startsWith(keyPrefix)) _memCache.delete(k)
  }
  for (let i = localStorage.length - 1; i >= 0; i--) {
    const lsKey = localStorage.key(i)
    if (!lsKey?.startsWith(LS_NAMESPACE)) continue
    const cacheKey = extractCacheKey(lsKey)
    if (cacheKey?.startsWith(keyPrefix)) localStorage.removeItem(lsKey)
  }
}

/** Clear all cached data for a given identity (memCache + localStorage). Call on logout. */
export function clearCacheForIdentity(identity: string): void {
  const identityPrefix = `${LS_NAMESPACE}${identity}:`
  for (let i = localStorage.length - 1; i >= 0; i--) {
    const k = localStorage.key(i)
    if (k?.startsWith(identityPrefix)) localStorage.removeItem(k)
  }
  _memCache.clear()
}

export interface CacheOpts {
  staleMs?: number
  /** Write to / hydrate from localStorage when true. Requires non-null identity. */
  persist?: boolean
  /** Namespace localStorage keys by user identity. null disables persist. */
  identity?: string | null
}

/**
 * A Solid-reactive resource with in-memory stale-while-revalidate semantics.
 * `loading` and `isStale` are reactive getters — read them inside JSX or effects
 * the same way you would read Solid's built-in `Resource.loading`.
 */
export type CachedResource<T> = {
  (): T | undefined
  readonly loading: boolean
  readonly error: unknown
  readonly isStale: boolean
  refetch(): void
}

/**
 * Best-effort background prefetch — fires the fetcher and writes to memCache.
 * No-op if the key is already cached. Errors are silently discarded.
 * Used by route-prefetch hooks to warm the cache before navigation.
 */
export function prefetch<T>(key: string, fetcher: () => Promise<T>): void {
  if (_memCache.has(key)) return
  fetcher().then(data => {
    _memCache.set(key, { data, updatedAt: Date.now() })
  }).catch(() => { /* best-effort prefetch */ })
}

export function useCachedResource<T>(
  key: () => string | undefined,
  fetcher: (k: string) => Promise<T>,
  opts?: CacheOpts,
): CachedResource<T> {
  const staleMs = opts?.staleMs ?? DEFAULT_STALE_MS
  const persist = opts?.persist ?? false
  const identity = opts?.identity ?? null

  const [data, setData] = createSignal<T | undefined>(undefined)
  const [loading, setLoading] = createSignal(false)
  const [error, setError] = createSignal<unknown>(undefined)
  const [isStale, setIsStale] = createSignal(false)

  async function doFetch(fetchKey: string, background: boolean) {
    if (!background) {
      setLoading(true)
      setError(undefined)
    }
    try {
      const result = await fetcher(fetchKey)
      const entry: CacheEntry<unknown> = { data: result, updatedAt: Date.now() }
      _memCache.set(fetchKey, entry)
      if (persist && identity) lsWrite(identity, fetchKey, entry)
      if (untrack(key) === fetchKey) {
        setData(() => result as NonNullable<T>)
        setIsStale(false)
      }
    } catch (e) {
      if (!background && untrack(key) === fetchKey) setError(e)
    } finally {
      if (!background && untrack(key) === fetchKey) setLoading(false)
    }
  }

  createEffect(() => {
    const k = key()
    if (k === undefined) return

    const cached = _memCache.get(k) as CacheEntry<T> | undefined
    if (cached) {
      setData(() => cached.data as NonNullable<T>)
      const age = Date.now() - cached.updatedAt
      if (age < staleMs) {
        setIsStale(false)
        setLoading(false)
        return
      }
      setIsStale(true)
      void doFetch(k, true)
    } else {
      // Try localStorage hydration on cold read
      if (persist && identity) {
        const persisted = lsRead(identity, k)
        if (persisted) {
          _memCache.set(k, persisted)
          setData(() => (persisted.data as NonNullable<T>))
          setIsStale(true)
          void doFetch(k, true)
          return
        }
      }
      setData(undefined)
      setIsStale(false)
      void doFetch(k, false)
    }
  })

  function refetch() {
    const k = untrack(key)
    if (k !== undefined) {
      _memCache.delete(k)
      if (persist && identity) localStorage.removeItem(buildLsKey(identity, k))
      setIsStale(false)
      void doFetch(k, false)
    }
  }

  const accessor = () => data() as T | undefined
  Object.defineProperty(accessor, 'loading', { get: () => loading(), enumerable: true })
  Object.defineProperty(accessor, 'error', { get: () => error(), enumerable: true })
  Object.defineProperty(accessor, 'isStale', { get: () => isStale(), enumerable: true })
  Object.defineProperty(accessor, 'refetch', { value: refetch, enumerable: true, writable: false })

  return accessor as unknown as CachedResource<T>
}
