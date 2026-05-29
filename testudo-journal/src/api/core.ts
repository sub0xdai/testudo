/** @anchor api:journal:core
 * @tags api */

export const API_BASE = import.meta.env.VITE_API_URL || ''

export interface StatsFilter {
    exchange?: string
    symbol?: string
    dateFrom?: string
    dateTo?: string
}

/**
 * Authenticated fetch with transparent token refresh on 401.
 *
 * Only safe/idempotent methods (GET, HEAD, OPTIONS) are retried after
 * token refresh. Mutations (POST, PUT, DELETE, PATCH) surface the
 * 401 error to the caller, which should trigger a re-auth flow.
 */
export async function fetchWithCredentials(
    url: string,
    init?: RequestInit,
): Promise<Response> {
    const opts: RequestInit = { ...init, credentials: 'include' }
    let res = await fetch(url, opts)
    if (res.status === 401) {
        const refreshRes = await fetch(`${API_BASE}/api/v1/auth/refresh`, {
            method: 'POST',
            credentials: 'include',
        })
        if (!refreshRes.ok) throw new Error('Session expired')
        // Only retry safe/idempotent methods. FormData/ReadableStream
        // bodies are consumed on first fetch and cannot be retried.
        const method = (init?.method ?? 'GET').toUpperCase()
        if (method === 'GET' || method === 'HEAD' || method === 'OPTIONS') {
            res = await fetch(url, opts)
        } else {
            throw new Error('Session expired — please re-authenticate')
        }
    }
    return res
}

export function buildParams(filters: StatsFilter): URLSearchParams {
    const params = new URLSearchParams()
    if (filters.exchange) params.set('exchange', filters.exchange)
    if (filters.symbol) params.set('symbol', filters.symbol)
    if (filters.dateFrom) params.set('date_from', filters.dateFrom)
    if (filters.dateTo) params.set('date_to', filters.dateTo)
    return params
}

export async function fetchApi<T>(
    path: string,
    filters: StatsFilter,
): Promise<T> {
    const params = buildParams(filters)
    const res = await fetchWithCredentials(
        `${API_BASE}/api/v1/journal/analytics/${path}?${params}`,
    )
    if (!res.ok) throw new Error(`API error: ${res.status}`)
    return res.json()
}

export async function fetchCrud<T>(
    path: string,
    init?: RequestInit,
): Promise<T> {
    const res = await fetchWithCredentials(
        `${API_BASE}/api/v1/journal/${path}`,
        {
            headers: { 'Content-Type': 'application/json' },
            ...init,
        },
    )
    if (!res.ok) throw new Error(`API error: ${res.status}`)
    return res.json()
}

export async function fetchExchange<T>(
    path: string,
    init?: RequestInit,
): Promise<T> {
    const res = await fetchWithCredentials(
        `${API_BASE}/api/v1/exchanges${path}`,
        {
            ...init,
            headers: { 'Content-Type': 'application/json', ...init?.headers },
        },
    )
    if (!res.ok) {
        const text = await res.text().catch(() => '')
        throw new Error(text || `Exchange API error: ${res.status}`)
    }
    return res.json()
}
