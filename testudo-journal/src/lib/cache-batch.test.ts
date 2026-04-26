import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'
import { createRoot } from 'solid-js'
import {
  useCachedResource,
  useCachedBatch,
  cacheKeyForSection,
  prime,
  stableHash,
  _memCache,
  type CachedResource,
  type SectionKey,
} from './cache'
import * as client from '../api/client'
import type { StatsFilter } from '../api/client'

/** Flush Solid's microtask-scheduled effects and async fetcher resolutions. */
async function tick(n = 5) {
  for (let i = 0; i < n; i++) await Promise.resolve()
}

// Same Map-backed localStorage shim as cache.test.ts.
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

const baseFilter: StatsFilter = { exchange: 'woo', symbol: 'BTC_USDT' }

// Concrete-but-minimal stand-ins for the section payloads (only the shapes
// the cache cares about — parity with the real wire format is enforced by
// the Rust integration test, not here).
function makeBatchResponse(): client.BatchAnalyticsResponse {
  return {
    overview: { account: { total_pnl: '1' } } as unknown as client.OverviewResponse,
    equity_curve: { data: [{ date: '2026-04-01' }] as unknown as client.EquityPoint[] },
    daily_pnl: { data: [{ date: '2026-04-01' }] as unknown as client.DailyPnlPoint[] },
    symbol_breakdown: { data: [{ symbol: 'BTC' }] as unknown as client.SymbolBreakdownItem[] },
    setup_breakdown: { data: [{ setup_tag: 'fade' }] as unknown as client.SetupBreakdownItem[] },
    duration_profit: { data: [{ duration_secs: 60 }] as unknown as client.DurationProfitPoint[] },
    return_distribution: { data: [{ bucket: '0–1R' }] as unknown as client.ReturnBucket[] },
    time_distribution: { data: [{ day_of_week: 1 }] as unknown as client.TimeSlot[] },
  }
}

// ---------------------------------------------------------------------------
// cacheKeyForSection — wire-format parity (defends spec risk #5)
// ---------------------------------------------------------------------------

describe('cacheKeyForSection', () => {
  it("produces 'overview:' + stableHash(filter)", () => {
    expect(cacheKeyForSection('overview', baseFilter)).toBe(
      'overview:' + stableHash(baseFilter),
    )
  })

  it('uses hyphenated wire prefix for multi-word sections', () => {
    expect(cacheKeyForSection('equity_curve', baseFilter)).toBe(
      'equity-curve:' + stableHash(baseFilter),
    )
    expect(cacheKeyForSection('symbol_breakdown', baseFilter)).toBe(
      'symbol-breakdown:' + stableHash(baseFilter),
    )
    expect(cacheKeyForSection('return_distribution', baseFilter)).toBe(
      'return-distribution:' + stableHash(baseFilter),
    )
  })

  it('matches cross-path keys for identical filters', () => {
    // The per-section call sites historically built keys as
    // `'<wire>:' + stableHash(filter)`. cacheKeyForSection must produce the
    // identical string so a batch-primed entry is read by per-section consumers.
    const filter: StatsFilter = { dateFrom: '2026-04-01', dateTo: '2026-04-30' }
    expect(cacheKeyForSection('daily_pnl', filter)).toBe(
      'daily-pnl:' + stableHash(filter),
    )
  })
})

// ---------------------------------------------------------------------------
// prime — narrow primitive used by useCachedBatch
// ---------------------------------------------------------------------------

describe('prime', () => {
  it('writes to memCache', () => {
    prime('overview:abc', { v: 42 })
    const entry = _memCache.get('overview:abc')
    expect(entry).toBeDefined()
    expect(entry!.data).toEqual({ v: 42 })
  })

  it('writes to localStorage when persist=true with identity', () => {
    prime('daily-pnl:xyz', { d: 'data' }, { persist: true, identity: 'user-a' })
    const raw = localStorage.getItem('testudo:cache:user-a:daily-pnl:xyz')
    expect(raw).not.toBeNull()
    const entry = JSON.parse(raw!)
    expect(entry.data).toEqual({ d: 'data' })
  })

  it('does NOT persist when identity is null', () => {
    prime('k', 1, { persist: true, identity: null })
    expect(localStorage.length).toBe(0)
  })

  it('is a no-op when an existing entry is fresher', () => {
    const future = Date.now() + 60_000
    _memCache.set('k', { data: 'fresh', updatedAt: future })
    prime('k', 'stale-overwrite-attempt')
    expect(_memCache.get('k')!.data).toBe('fresh')
  })

  it("primed entry is read by useCachedResource without triggering its own fetcher", async () => {
    prime('overview:' + stableHash(baseFilter), { account: { id: 1 } })
    const fetcher = vi.fn().mockResolvedValue('SHOULD-NOT-FIRE')
    let resource!: CachedResource<unknown>
    const dispose = createRoot(d => {
      resource = useCachedResource(
        () => 'overview:' + stableHash(baseFilter),
        () => fetcher(),
        { staleMs: 30_000 },
      )
      return d
    })
    await tick()
    expect(fetcher).not.toHaveBeenCalled()
    expect(resource()).toEqual({ account: { id: 1 } })
    dispose()
  })
})

// ---------------------------------------------------------------------------
// useCachedBatch — partition + fan-out
// ---------------------------------------------------------------------------

describe('useCachedBatch', () => {
  it('all-cold: issues exactly one batched fetch for all requested sections', async () => {
    const spy = vi.spyOn(client, 'fetchAnalyticsBatch').mockResolvedValue(makeBatchResponse())
    const requested: SectionKey[] = ['overview', 'equity_curve', 'daily_pnl']

    const dispose = createRoot(d => {
      useCachedBatch(() => requested, () => baseFilter)
      return d
    })

    await tick(5)

    expect(spy).toHaveBeenCalledTimes(1)
    // Ensure the *requested* sections went over the wire — order is irrelevant.
    const [sentSections, sentFilter] = spy.mock.calls[0]
    expect(new Set(sentSections!)).toEqual(new Set(requested))
    expect(sentFilter).toEqual(baseFilter)

    dispose()
  })

  it('all-warm: zero fetches when every section is primed within staleMs', async () => {
    const spy = vi.spyOn(client, 'fetchAnalyticsBatch').mockResolvedValue(makeBatchResponse())
    const requested: SectionKey[] = ['overview', 'equity_curve', 'daily_pnl']
    for (const s of requested) {
      prime(cacheKeyForSection(s, baseFilter), { ok: true })
    }

    const dispose = createRoot(d => {
      useCachedBatch(() => requested, () => baseFilter, { staleMs: 30_000 })
      return d
    })

    await tick(5)

    expect(spy).not.toHaveBeenCalled()

    dispose()
  })

  it('mixed: prime 4 of 7 → batch fetches only the 3 stale sections', async () => {
    const spy = vi.spyOn(client, 'fetchAnalyticsBatch').mockResolvedValue(makeBatchResponse())
    const all: SectionKey[] = [
      'overview', 'equity_curve', 'daily_pnl', 'symbol_breakdown',
      'setup_breakdown', 'duration_profit', 'return_distribution',
    ]
    const warm: SectionKey[] = ['overview', 'equity_curve', 'daily_pnl', 'symbol_breakdown']
    for (const s of warm) {
      prime(cacheKeyForSection(s, baseFilter), { ok: true })
    }

    const dispose = createRoot(d => {
      useCachedBatch(() => all, () => baseFilter, { staleMs: 30_000 })
      return d
    })

    await tick(5)

    expect(spy).toHaveBeenCalledTimes(1)
    const [sentSections] = spy.mock.calls[0]
    expect(new Set(sentSections!)).toEqual(
      new Set<SectionKey>(['setup_breakdown', 'duration_profit', 'return_distribution']),
    )

    dispose()
  })

  it('per-section error: errored section is NOT primed, others are', async () => {
    const partial: client.BatchAnalyticsResponse = {
      overview: { error: 'simulated overview failure' },
      equity_curve: { data: [{ date: '2026-04-01' }] as unknown as client.EquityPoint[] },
    }
    const spy = vi.spyOn(client, 'fetchAnalyticsBatch').mockResolvedValue(partial)

    const dispose = createRoot(d => {
      useCachedBatch(
        () => ['overview', 'equity_curve'],
        () => baseFilter,
      )
      return d
    })

    await tick(5)

    expect(spy).toHaveBeenCalledTimes(1)
    // equity_curve was primed
    expect(_memCache.has(cacheKeyForSection('equity_curve', baseFilter))).toBe(true)
    // overview was NOT primed (errored)
    expect(_memCache.has(cacheKeyForSection('overview', baseFilter))).toBe(false)

    dispose()
  })

  it('refetch: clears entries and re-issues the batch', async () => {
    const spy = vi.spyOn(client, 'fetchAnalyticsBatch').mockResolvedValue(makeBatchResponse())
    const requested: SectionKey[] = ['overview', 'equity_curve']

    let batchHandle!: ReturnType<typeof useCachedBatch>
    const dispose = createRoot(d => {
      batchHandle = useCachedBatch(() => requested, () => baseFilter, { staleMs: 60_000 })
      return d
    })

    await tick(5)
    expect(spy).toHaveBeenCalledTimes(1)

    // Within staleMs: a no-op effect re-run wouldn't fetch. refetch() must.
    batchHandle.refetch()
    await tick(5)
    expect(spy).toHaveBeenCalledTimes(2)

    dispose()
  })
})
