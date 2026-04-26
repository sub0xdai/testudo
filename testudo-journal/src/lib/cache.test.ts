import { describe, it, expect, vi, beforeEach } from 'vitest'
import { createRoot } from 'solid-js'
import { useCachedResource, invalidate, stableHash, _memCache, type CachedResource } from './cache'

/** Flush Solid's microtask-scheduled effects and async fetcher resolutions. */
async function tick(n = 5) {
  for (let i = 0; i < n; i++) await Promise.resolve()
}

beforeEach(() => {
  _memCache.clear()
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
