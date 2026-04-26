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

/** Drop all cache entries whose key starts with `keyPrefix`. */
export function invalidate(keyPrefix: string): void {
  for (const k of [..._memCache.keys()]) {
    if (k.startsWith(keyPrefix)) _memCache.delete(k)
  }
}

/** CP-4: also clears identity-namespaced localStorage entries. Stub for CP-3. */
export function clearCacheForIdentity(_identity: string): void {
  // Implemented in CP-4 — localStorage namespace cleanup goes here.
}

export interface CacheOpts {
  staleMs?: number
  /** CP-4: write to / hydrate from localStorage when true. */
  persist?: boolean
  /** CP-4: namespace localStorage keys by user identity. null disables persist. */
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

export function useCachedResource<T>(
  key: () => string | undefined,
  fetcher: (k: string) => Promise<T>,
  opts?: CacheOpts,
): CachedResource<T> {
  const staleMs = opts?.staleMs ?? DEFAULT_STALE_MS

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
      _memCache.set(fetchKey, { data: result, updatedAt: Date.now() })
      // Only update component state if the key is still current.
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
      // Stale: serve immediately, revalidate in the background.
      setIsStale(true)
      void doFetch(k, true)
    } else {
      setData(undefined)
      setIsStale(false)
      void doFetch(k, false)
    }
  })

  function refetch() {
    const k = untrack(key)
    if (k !== undefined) {
      _memCache.delete(k)
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
