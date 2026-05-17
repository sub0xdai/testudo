import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest'
import {
  fetchOverview,
  fetchEquityCurve,
  fetchTrades,
  fetchTags,
  fetchFilterOptions,
  fetchTradeDetail,
  type StatsFilter,
} from './client'

// --- Helpers ---

function jsonResponse(body: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
    headers: new Headers(),
    redirected: false,
    statusText: status === 200 ? 'OK' : 'Error',
    type: 'basic' as ResponseType,
    url: '',
    clone: () => jsonResponse(body, status) as Response,
    body: null,
    bodyUsed: false,
    arrayBuffer: () => Promise.resolve(new ArrayBuffer(0)),
    blob: () => Promise.resolve(new Blob()),
    formData: () => Promise.resolve(new FormData()),
    text: () => Promise.resolve(JSON.stringify(body)),
    bytes: () => Promise.resolve(new Uint8Array()),
  } as Response
}

// --- Setup ---

let mockFetch: Mock

beforeEach(() => {
  mockFetch = vi.fn()
  vi.stubGlobal('fetch', mockFetch)
})

// --- Tests ---

describe('fetchWithCredentials (tested via exported API functions)', () => {
  it('sends credentials: "include" on every request', async () => {
    const payload = { account: {}, performance: {}, risk: {} }
    mockFetch.mockResolvedValue(jsonResponse(payload))

    await fetchOverview({})

    expect(mockFetch).toHaveBeenCalledTimes(1)
    const [, init] = mockFetch.mock.calls[0]
    expect(init.credentials).toBe('include')
  })

  it('on 401: calls POST /api/v1/auth/refresh, then retries the original request', async () => {
    const tagsPayload = [{ id: '1', user_id: 'u1', name: 'scalp', color: null }]

    // First call: 401 unauthorized
    mockFetch.mockResolvedValueOnce(jsonResponse({}, 401))
    // Second call: refresh succeeds
    mockFetch.mockResolvedValueOnce(jsonResponse({}, 200))
    // Third call: retry succeeds
    mockFetch.mockResolvedValueOnce(jsonResponse(tagsPayload))

    const result = await fetchTags()

    expect(result).toEqual(tagsPayload)
    expect(mockFetch).toHaveBeenCalledTimes(3)

    // Call 1: original request
    const [url1, init1] = mockFetch.mock.calls[0]
    expect(url1).toContain('/api/v1/journal/tags')
    expect(init1.credentials).toBe('include')

    // Call 2: refresh
    const [url2, init2] = mockFetch.mock.calls[1]
    expect(url2).toBe('/api/v1/auth/refresh')
    expect(init2.method).toBe('POST')
    expect(init2.credentials).toBe('include')

    // Call 3: retry of original
    const [url3, init3] = mockFetch.mock.calls[2]
    expect(url3).toContain('/api/v1/journal/tags')
    expect(init3.credentials).toBe('include')
  })

  it('on 401 + refresh fails: throws "Session expired"', async () => {
    // First call: 401
    mockFetch.mockResolvedValueOnce(jsonResponse({}, 401))
    // Second call: refresh also fails
    mockFetch.mockResolvedValueOnce(jsonResponse({}, 403))

    await expect(fetchTags()).rejects.toThrow('Session expired')
    expect(mockFetch).toHaveBeenCalledTimes(2)
  })
})

describe('buildParams (tested via fetchTrades)', () => {
  it('serializes page, limit, exchange, symbol, dateFrom, dateTo, sort, order — skipping undefined values', async () => {
    const tradesPayload = { trades: [], total: 0, page: 1, limit: 25 }
    mockFetch.mockResolvedValue(jsonResponse(tradesPayload))

    await fetchTrades({
      page: 2,
      limit: 50,
      exchange: 'binance',
      symbol: 'BTCUSDT',
      dateFrom: '2026-01-01',
      dateTo: '2026-03-25',
      sort: 'closed_at',
      order: 'desc',
    })

    const [url] = mockFetch.mock.calls[0]
    const parsedUrl = new URL(url, 'http://localhost')
    expect(parsedUrl.searchParams.get('page')).toBe('2')
    expect(parsedUrl.searchParams.get('limit')).toBe('50')
    expect(parsedUrl.searchParams.get('exchange')).toBe('binance')
    expect(parsedUrl.searchParams.get('symbol')).toBe('BTCUSDT')
    expect(parsedUrl.searchParams.get('date_from')).toBe('2026-01-01')
    expect(parsedUrl.searchParams.get('date_to')).toBe('2026-03-25')
    expect(parsedUrl.searchParams.get('sort')).toBe('closed_at')
    expect(parsedUrl.searchParams.get('order')).toBe('desc')
  })

  it('handles empty/all-undefined inputs producing no query params', async () => {
    const overviewPayload = { account: {}, performance: {}, risk: {} }
    mockFetch.mockResolvedValue(jsonResponse(overviewPayload))

    await fetchOverview({})

    const [url] = mockFetch.mock.calls[0]
    const parsedUrl = new URL(url, 'http://localhost')
    // No params should be set when the filter object is empty
    expect(parsedUrl.searchParams.toString()).toBe('')
  })

  it('skips empty string values in StatsFilter', async () => {
    const overviewPayload = { account: {}, performance: {}, risk: {} }
    mockFetch.mockResolvedValue(jsonResponse(overviewPayload))

    await fetchOverview({ exchange: '', symbol: '', dateFrom: '', dateTo: '' })

    const [url] = mockFetch.mock.calls[0]
    const parsedUrl = new URL(url, 'http://localhost')
    expect(parsedUrl.searchParams.toString()).toBe('')
  })
})

describe('fetchApi URL construction', () => {
  it('constructs correct analytics URL from path and filter params', async () => {
    const payload = { data: [] }
    mockFetch.mockResolvedValue(jsonResponse(payload))

    const filters: StatsFilter = { exchange: 'woo', dateFrom: '2026-03-01' }
    await fetchEquityCurve(filters)

    const [url] = mockFetch.mock.calls[0]
    expect(url).toContain('/api/v1/journal/analytics/equity-curve')
    const parsedUrl = new URL(url, 'http://localhost')
    expect(parsedUrl.searchParams.get('exchange')).toBe('woo')
    expect(parsedUrl.searchParams.get('date_from')).toBe('2026-03-01')
  })

  it('fetchFilterOptions constructs correct URL with optional exchange param', async () => {
    const payload = { exchanges: ['binance'], symbols: [] }
    mockFetch.mockResolvedValue(jsonResponse(payload))

    await fetchFilterOptions('binance')

    const [url] = mockFetch.mock.calls[0]
    expect(url).toContain('/api/v1/journal/analytics/filter-options')
    const parsedUrl = new URL(url, 'http://localhost')
    expect(parsedUrl.searchParams.get('exchange')).toBe('binance')
  })

  it('fetchCrud constructs correct journal CRUD URL', async () => {
    const detail = { id: 'abc', entries: [], tags: [] }
    mockFetch.mockResolvedValue(jsonResponse(detail))

    await fetchTradeDetail('abc')

    const [url] = mockFetch.mock.calls[0]
    expect(url).toContain('/api/v1/journal/trades/abc')
  })
})

describe('error handling', () => {
  it('throws on non-ok responses from analytics endpoints', async () => {
    mockFetch.mockResolvedValue(jsonResponse({}, 500))

    await expect(fetchOverview({})).rejects.toThrow('API error: 500')
  })

  it('throws on non-ok responses from CRUD endpoints', async () => {
    mockFetch.mockResolvedValue(jsonResponse({}, 404))

    await expect(fetchTradeDetail('nonexistent')).rejects.toThrow('API error: 404')
  })

  it('throws on non-ok responses from filter options endpoint', async () => {
    mockFetch.mockResolvedValue(jsonResponse({}, 503))

    await expect(fetchFilterOptions()).rejects.toThrow('API error: 503')
  })
})
