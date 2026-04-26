import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'
import { createRoot } from 'solid-js'
import { useCachedResource, invalidate, clearCacheForIdentity, stableHash, _memCache, type CachedResource } from './cache'

/** Flush Solid's microtask-scheduled effects and async fetcher resolutions. */
async function tick(n = 5) {
  for (let i = 0; i < n; i++) await Promise.resolve()
}

// Bun's runtime overrides jsdom's localStorage with a limited implementation.
// Provide a full Map-backed mock so tests don't depend on environment storage.
const _store: Record<string, string> = {}
const mockLocalStorage = {
  get length() { return Object.keys(_store).length },
  getItem(k: string): string | null { return Object.prototype.hasOwnProperty.call(_store, k) ? _store[k] : null },
  setItem(k: string, v: string): void { _store[k] = v },
  removeItem(k: string): void { delete _store[k] },
  key(index: number): string | null { return Object.keys(_store)[index] ?? null },
  clear(): void { Object.keys(_store).forEach(k => delete _store[k]) },
}

beforeAll(() => {
  vi.stubGlobal('localStorage', mockLocalStorage)
})

beforeEach(() => {
  _memCache.clear()
  Object.keys(_store).forEach(k => delete _store[k])
  vi.restoreAllMocks()
})

// ---------------------------------------------------------------------------
// stableHash
// ---------------------------------------------------------------------------

describe('stableHash', () => {
  it('is deterministic regardless of key order', () => {
    expect(stableHash({ a: 1, b: 2 })).toBe(stableHash({ b: 2, a: 1 }))
  })

  it('differentiates different values', () => {
    expect(stableHash({ a: 1 })).not.toBe(stableHash({ a: 2 }))
  })

  it('treats undefined values as absent (same as empty object)', () => {
    expect(stableHash({ a: undefined })).toBe(stableHash({}))
  })

  it('handles nested objects with unsorted keys', () => {
    expect(stableHash({ x: { a: 1, b: 2 } })).toBe(stableHash({ x: { b: 2, a: 1 } }))
  })

  it('handles arrays without sorting (order matters)', () => {
    expect(stableHash([1, 2])).not.toBe(stableHash([2, 1]))
  })
})

// ---------------------------------------------------------------------------
// invalidate
// ---------------------------------------------------------------------------

describe('invalidate', () => {
  it('removes all cache entries with matching prefix', () => {
    _memCache.set('tags:all', { data: [], updatedAt: Date.now() })
    _memCache.set('tags:user', { data: [], updatedAt: Date.now() })
    _memCache.set('overview:{}', { data: {}, updatedAt: Date.now() })

    invalidate('tags:')

    expect(_memCache.has('tags:all')).toBe(false)
    expect(_memCache.has('tags:user')).toBe(false)
    expect(_memCache.has('overview:{}')).toBe(true)
  })

  it('is a no-op when no keys match', () => {
    _memCache.set('overview:{}', { data: {}, updatedAt: Date.now() })
    invalidate('nonexistent:')
    expect(_memCache.has('overview:{}')).toBe(true)
  })
})

// ---------------------------------------------------------------------------
// useCachedResource — core behaviours
// ---------------------------------------------------------------------------

describe('useCachedResource', () => {
  it('cold miss: calls fetcher and stores result in memCache', async () => {
    const fetcher = vi.fn().mockResolvedValue('result')
    let resource!: CachedResource<string>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => 'cold-key', () => fetcher('cold-key'))
      return d
    })

    await tick()

    expect(fetcher).toHaveBeenCalledTimes(1)
    expect(resource()).toBe('result')
    expect(_memCache.has('cold-key')).toBe(true)
    expect(resource.loading).toBe(false)

    dispose()
  })

  it('warm hit: skips fetcher when data is fresh', async () => {
    _memCache.set('warm-key', { data: 'cached', updatedAt: Date.now() })
    const fetcher = vi.fn().mockResolvedValue('fresh')
    let resource!: CachedResource<string>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => 'warm-key', () => fetcher('warm-key'), { staleMs: 30_000 })
      return d
    })

    await tick()

    expect(fetcher).not.toHaveBeenCalled()
    expect(resource()).toBe('cached')
    expect(resource.loading).toBe(false)

    dispose()
  })

  it('stale revalidate: returns stale data immediately and refetches in background', async () => {
    _memCache.set('stale-key', { data: 'old', updatedAt: Date.now() - 60_000 })

    // Use a deferred promise so we can inspect state before the fetch resolves.
    let resolveFetch!: (v: string) => void
    const deferred = new Promise<string>(r => { resolveFetch = r })
    const fetcher = vi.fn().mockReturnValue(deferred)

    let resource!: CachedResource<string>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => 'stale-key', () => fetcher('stale-key'), { staleMs: 30_000 })
      return d
    })

    // Effect has fired: stale data served, background fetch kicked off but not resolved.
    await tick(2)

    expect(resource()).toBe('old')
    expect(resource.isStale).toBe(true)
    expect(fetcher).toHaveBeenCalledTimes(1)

    // Resolve the background fetch — data should update.
    resolveFetch('fresh')
    await tick(5)

    expect(resource()).toBe('fresh')
    expect(resource.isStale).toBe(false)

    dispose()
  })

  it('isStale: is false for fresh cache entries', async () => {
    _memCache.set('fresh-key', { data: 'data', updatedAt: Date.now() })
    const fetcher = vi.fn().mockResolvedValue('new')
    let resource!: CachedResource<string>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => 'fresh-key', () => fetcher('fresh-key'), { staleMs: 30_000 })
      return d
    })

    await tick()

    expect(resource.isStale).toBe(false)

    dispose()
  })

  it('manual invalidation + refetch: drops entry and re-fetches', async () => {
    _memCache.set('inv-key', { data: 'old', updatedAt: Date.now() })
    const fetcher = vi.fn().mockResolvedValue('after-invalidate')
    let resource!: CachedResource<string>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => 'inv-key', () => fetcher('inv-key'), { staleMs: 30_000 })
      return d
    })

    await tick()
    expect(fetcher).not.toHaveBeenCalled()
    expect(resource()).toBe('old')

    invalidate('inv-key')
    resource.refetch()

    await tick(5)

    expect(fetcher).toHaveBeenCalledTimes(1)
    expect(resource()).toBe('after-invalidate')

    dispose()
  })

  it('undefined key: skips fetch and returns undefined', async () => {
    const fetcher = vi.fn().mockResolvedValue('result')
    let resource!: CachedResource<string>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => undefined, () => fetcher('key'))
      return d
    })

    await tick()

    expect(fetcher).not.toHaveBeenCalled()
    expect(resource()).toBeUndefined()

    dispose()
  })
})

// ---------------------------------------------------------------------------
// localStorage persistence tier (CP-4)
// ---------------------------------------------------------------------------

describe('localStorage persistence', () => {
  it('writes to localStorage after a successful fetch', async () => {
    const fetcher = vi.fn().mockResolvedValue({ value: 42 })
    const dispose = createRoot(d => {
      useCachedResource(() => 'ls-write-key', () => fetcher('ls-write-key'), {
        persist: true,
        identity: 'user-a',
      })
      return d
    })

    await tick()

    const raw = localStorage.getItem('testudo:cache:user-a:ls-write-key')
    expect(raw).not.toBeNull()
    const entry = JSON.parse(raw!)
    expect(entry.data).toEqual({ value: 42 })

    dispose()
  })

  it('hydrates from localStorage on cold memCache miss and returns stale data immediately', async () => {
    const staleData = { trades: 5 }
    localStorage.setItem(
      'testudo:cache:user-a:hydrate-key',
      JSON.stringify({ data: staleData, updatedAt: Date.now() - 60_000 }),
    )

    let resolveFetch!: (v: typeof staleData) => void
    const deferred = new Promise<typeof staleData>(r => { resolveFetch = r })
    const fetcher = vi.fn().mockReturnValue(deferred)

    let resource!: CachedResource<typeof staleData>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => 'hydrate-key', () => fetcher('hydrate-key'), {
        staleMs: 30_000,
        persist: true,
        identity: 'user-a',
      })
      return d
    })

    await tick(2)

    // Hydrated from localStorage — stale data served immediately
    expect(resource()).toEqual(staleData)
    expect(resource.isStale).toBe(true)
    expect(fetcher).toHaveBeenCalledTimes(1)

    // Resolve background fetch
    resolveFetch({ trades: 9 })
    await tick(5)

    expect(resource()).toEqual({ trades: 9 })
    expect(resource.isStale).toBe(false)

    dispose()
  })

  it('identity isolation: identity-B cannot read identity-A localStorage entries', async () => {
    localStorage.setItem(
      'testudo:cache:user-a:secret-key',
      JSON.stringify({ data: 'user-a-data', updatedAt: Date.now() }),
    )

    const fetcher = vi.fn().mockResolvedValue('user-b-fresh')
    let resource!: CachedResource<string>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => 'secret-key', () => fetcher('secret-key'), {
        persist: true,
        identity: 'user-b',
      })
      return d
    })

    await tick()

    // Must NOT serve user-a's data
    expect(resource()).toBe('user-b-fresh')
    expect(fetcher).toHaveBeenCalledTimes(1)

    dispose()
  })

  it('clearCacheForIdentity removes only that identity from localStorage and clears memCache', async () => {
    localStorage.setItem('testudo:cache:user-a:key1', JSON.stringify({ data: 1, updatedAt: 1 }))
    localStorage.setItem('testudo:cache:user-a:key2', JSON.stringify({ data: 2, updatedAt: 2 }))
    localStorage.setItem('testudo:cache:user-b:key1', JSON.stringify({ data: 3, updatedAt: 3 }))
    _memCache.set('key1', { data: 'mem', updatedAt: Date.now() })

    clearCacheForIdentity('user-a')

    expect(localStorage.getItem('testudo:cache:user-a:key1')).toBeNull()
    expect(localStorage.getItem('testudo:cache:user-a:key2')).toBeNull()
    expect(localStorage.getItem('testudo:cache:user-b:key1')).not.toBeNull()
    expect(_memCache.size).toBe(0)
  })

  it('invalidate also removes matching localStorage entries across all identities', () => {
    localStorage.setItem('testudo:cache:user-a:tags:all', JSON.stringify({ data: [], updatedAt: 1 }))
    localStorage.setItem('testudo:cache:user-b:tags:all', JSON.stringify({ data: [], updatedAt: 2 }))
    localStorage.setItem('testudo:cache:user-a:overview:{}', JSON.stringify({ data: {}, updatedAt: 3 }))

    invalidate('tags:')

    expect(localStorage.getItem('testudo:cache:user-a:tags:all')).toBeNull()
    expect(localStorage.getItem('testudo:cache:user-b:tags:all')).toBeNull()
    expect(localStorage.getItem('testudo:cache:user-a:overview:{}')).not.toBeNull()
  })

  it('null identity: does not persist to localStorage', async () => {
    const fetcher = vi.fn().mockResolvedValue('no-persist')
    const dispose = createRoot(d => {
      useCachedResource(() => 'null-id-key', () => fetcher('null-id-key'), {
        persist: true,
        identity: null,
      })
      return d
    })

    await tick()

    expect(localStorage.length).toBe(0)

    dispose()
  })

  it('quota exceeded: falls back to memory-only after two failures', async () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
    let callCount = 0
    vi.spyOn(localStorage, 'setItem').mockImplementation(() => {
      callCount++
      // Fail the first two attempts (initial write + retry after evict)
      if (callCount <= 2) throw new DOMException('QuotaExceededError', 'QuotaExceededError')
    })

    const fetcher = vi.fn().mockResolvedValue('data')
    let resource!: CachedResource<string>
    const dispose = createRoot(d => {
      resource = useCachedResource(() => 'quota-key', () => fetcher('quota-key'), {
        persist: true,
        identity: 'user-a',
      })
      return d
    })

    await tick()

    // Data still available in memCache even if localStorage failed
    expect(resource()).toBe('data')
    expect(warnSpy).toHaveBeenCalledWith(expect.stringContaining('[cache] localStorage quota exceeded'))

    dispose()
  })
})
