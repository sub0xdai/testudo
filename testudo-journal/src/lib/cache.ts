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
import type { StatsFilter } from '../api/client'
import { fetchAnalyticsBatch, type BatchAnalyticsResponse } from '../api/client'

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

// ---------------------------------------------------------------------------
// PERF-02 CP-2: SectionKey + cacheKeyForSection + prime + useCachedBatch
// ---------------------------------------------------------------------------

/**
 * Wire-format section identifiers. Snake_case here matches the Rust
 * `SectionKey` enum's `serde(rename_all = "snake_case")` representation —
 * see `testudo-exchange/crates/router/src/routes/journal.rs`.
 */
export type SectionKey =
  | 'overview'
  | 'equity_curve'
  | 'daily_pnl'
  | 'symbol_breakdown'
  | 'setup_breakdown'
  | 'duration_profit'
  | 'return_distribution'
  | 'time_distribution'

/**
 * Map a SectionKey to its hyphenated cache-key prefix. Per-section call sites
 * historically used keys like `'equity-curve:' + stableHash(filter)`; this
 * helper centralizes the wire format so batch and per-section paths cannot
 * drift (defends spec risk #5).
 */
const SECTION_KEY_PREFIX: Record<SectionKey, string> = {
  overview: 'overview',
  equity_curve: 'equity-curve',
  daily_pnl: 'daily-pnl',
  symbol_breakdown: 'symbol-breakdown',
  setup_breakdown: 'setup-breakdown',
  duration_profit: 'duration-profit',
  return_distribution: 'return-distribution',
  time_distribution: 'time-distribution',
}

export function cacheKeyForSection(key: SectionKey, filter: StatsFilter): string {
  return `${SECTION_KEY_PREFIX[key]}:${stableHash(filter)}`
}

/**
 * Narrow primitive: write `{ data, updatedAt: Date.now() }` into memCache and
 * (optionally) localStorage. No-op if an existing entry is fresher — protects
 * us from clobbering a recent per-section fetch with a slower batch response.
 */
export function prime<T>(
  key: string,
  data: T,
  opts?: { identity?: string | null; persist?: boolean },
): void {
  const now = Date.now()
  const existing = _memCache.get(key)
  if (existing && existing.updatedAt >= now) return // already fresher
  const entry: CacheEntry<unknown> = { data, updatedAt: now }
  _memCache.set(key, entry)
  if (opts?.persist && opts.identity) lsWrite(opts.identity, key, entry)
}

export interface BatchOpts {
  staleMs?: number
  persist?: boolean
  identity?: string | null
}

export type BatchSections = Record<SectionKey, CachedResource<unknown>>

export interface UseCachedBatchResult {
  sections: BatchSections
  anyLoading: () => boolean
  refetch: () => void
}

/**
 * A SWR-style fan-out hook: requests N analytics sections in one batched POST,
 * partitioning requested sections into FRESH (memCache age < staleMs, no
 * network) and STALE_OR_MISSING (one batched fetch). On response, primes each
 * section's individual cache key so per-section consumers (PnlTreemap, DailyPnl
 * chart, etc.) see cache hits and skip their own fetches.
 *
 * Per-section error envelopes (`{ error: '...' }`) are NOT primed — any stale
 * entry is left intact, mirroring `useCachedResource`'s render-stale-on-error
 * semantics.
 */
export function useCachedBatch(
  sections: () => SectionKey[],
  filter: () => StatsFilter,
  opts?: BatchOpts,
): UseCachedBatchResult {
  const staleMs = opts?.staleMs ?? DEFAULT_STALE_MS
  const persist = opts?.persist ?? false
  const identity = opts?.identity ?? null

  // Per-section reactive backing signals. We allocate signals for the whole
  // SectionKey set up front so the returned `sections` record is stable.
  const ALL_KEYS: SectionKey[] = [
    'overview', 'equity_curve', 'daily_pnl', 'symbol_breakdown',
    'setup_breakdown', 'duration_profit', 'return_distribution', 'time_distribution',
  ]

  type Slot = {
    data: ReturnType<typeof createSignal<unknown>>
    isStale: ReturnType<typeof createSignal<boolean>>
    error: ReturnType<typeof createSignal<unknown>>
  }
  const slots = new Map<SectionKey, Slot>()
  for (const k of ALL_KEYS) {
    slots.set(k, {
      data: createSignal<unknown>(undefined),
      isStale: createSignal<boolean>(false),
      error: createSignal<unknown>(undefined),
    })
  }

  const [loading, setLoading] = createSignal(false)
  // Bumped by `refetch()` to force the effect to re-run even when keys are unchanged.
  const [refetchTick, setRefetchTick] = createSignal(0)

  function readSlotFromCache(section: SectionKey, key: string): boolean {
    const slot = slots.get(section)!
    const cached = _memCache.get(key) as CacheEntry<unknown> | undefined
    if (cached) {
      slot.data[1](() => cached.data)
      const age = Date.now() - cached.updatedAt
      slot.isStale[1](age >= staleMs)
      return age < staleMs
    }
    // Try localStorage hydration on cold read.
    if (persist && identity) {
      const persisted = lsRead(identity, key)
      if (persisted) {
        _memCache.set(key, persisted)
        slot.data[1](() => persisted.data)
        slot.isStale[1](true)
        return false
      }
    }
    slot.data[1](undefined)
    slot.isStale[1](false)
    return false
  }

  function isErrorPayload(payload: unknown): payload is { error: string } {
    return (
      typeof payload === 'object' &&
      payload !== null &&
      'error' in payload &&
      typeof (payload as { error: unknown }).error === 'string'
    )
  }

  async function runBatch(stale: SectionKey[], capturedFilter: StatsFilter) {
    if (stale.length === 0) return
    setLoading(true)
    try {
      const response = await fetchAnalyticsBatch(stale, capturedFilter)
      // Snapshot the current keys derived from `capturedFilter` — the user
      // could change filters mid-flight; in that case we still prime under
      // the captured-filter key so a future read with the same filter hits.
      for (const section of stale) {
        const payload = (response as Record<string, unknown>)[section]
        if (payload === undefined || payload === null) continue
        if (isErrorPayload(payload)) {
          slots.get(section)!.error[1](() => new Error(payload.error))
          continue
        }
        const key = cacheKeyForSection(section, capturedFilter)
        prime(key, payload, { identity, persist })
        const slot = slots.get(section)!
        // Only update the visible slot if filter is still the same (the data
        // belongs to the captured filter; changing filters reroutes via the
        // effect re-running).
        if (untrack(filter) === capturedFilter || stableHash(untrack(filter)) === stableHash(capturedFilter)) {
          slot.data[1](() => payload)
          slot.isStale[1](false)
          slot.error[1](() => undefined)
        }
      }
    } catch (e) {
      // Network-level failure: surface error on every requested section so
      // consumers can render an error state. Stale entries (if any) remain.
      for (const section of stale) {
        slots.get(section)!.error[1](() => e)
      }
    } finally {
      setLoading(false)
    }
  }

  createEffect(() => {
    refetchTick() // reactive dep — bumped by refetch()
    const requested = sections()
    const f = filter()
    const stale: SectionKey[] = []
    for (const section of requested) {
      const key = cacheKeyForSection(section, f)
      const fresh = readSlotFromCache(section, key)
      if (!fresh) stale.push(section)
    }
    if (stale.length > 0) {
      void runBatch(stale, f)
    }
  })

  function refetch() {
    // Drop memCache entries for the currently requested sections so the
    // effect treats them as cold misses on the next run.
    const requested = untrack(sections)
    const f = untrack(filter)
    for (const section of requested) {
      const key = cacheKeyForSection(section, f)
      _memCache.delete(key)
      if (persist && identity) localStorage.removeItem(buildLsKey(identity, key))
      const slot = slots.get(section)!
      slot.error[1](() => undefined)
    }
    setRefetchTick(t => t + 1)
  }

  // Build the public Record<SectionKey, CachedResource<unknown>> facade.
  // Each accessor reads its slot signals; refetch on a per-section accessor
  // forwards to the batch refetch (slim — sections are co-fetched by design).
  const out = {} as BatchSections
  for (const k of ALL_KEYS) {
    const slot = slots.get(k)!
    const accessor = () => slot.data[0]()
    Object.defineProperty(accessor, 'loading', { get: () => loading(), enumerable: true })
    Object.defineProperty(accessor, 'error', { get: () => slot.error[0](), enumerable: true })
    Object.defineProperty(accessor, 'isStale', { get: () => slot.isStale[0](), enumerable: true })
    Object.defineProperty(accessor, 'refetch', { value: refetch, enumerable: true, writable: false })
    ;(out as Record<string, unknown>)[k] = accessor as unknown as CachedResource<unknown>
  }

  return {
    sections: out,
    anyLoading: () => loading(),
    refetch,
  }
}

// Re-export for type ergonomics on the consuming side.
export type { BatchAnalyticsResponse }
