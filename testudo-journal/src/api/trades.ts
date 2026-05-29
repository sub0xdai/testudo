/** @anchor api:journal:trades
 * @tags api */

import { fetchCrud, fetchWithCredentials, API_BASE } from './core'
import type { JournalTrade, JournalTag, JournalEntry } from './types'

export interface TradeDetail extends JournalTrade {
    entries: JournalEntry[]
    tags: JournalTag[]
}

export interface TradeWithTags extends JournalTrade {
    tags: JournalTag[]
}

export interface TradesResponse {
    trades: TradeWithTags[]; total: number; page: number; limit: number
}

export interface TradeListParams {
    page?: number; limit?: number; exchange?: string; symbol?: string
    side?: string; tag?: string; dateFrom?: string; dateTo?: string
    sort?: string; order?: string
}

export async function fetchTrades(params: TradeListParams): Promise<TradesResponse> {
    const p = new URLSearchParams()
    if (params.page) p.set('page', String(params.page))
    if (params.limit) p.set('limit', String(params.limit))
    if (params.exchange) p.set('exchange', params.exchange)
    if (params.symbol) p.set('symbol', params.symbol)
    if (params.side) p.set('side', params.side)
    if (params.tag) p.set('tag', params.tag)
    if (params.dateFrom) p.set('date_from', params.dateFrom)
    if (params.dateTo) p.set('date_to', params.dateTo)
    if (params.sort) p.set('sort', params.sort)
    if (params.order) p.set('order', params.order)
    return fetchCrud<TradesResponse>(`trades?${p}`)
}

export async function fetchTradeDetail(tradeId: string): Promise<TradeDetail> {
    return fetchCrud<TradeDetail>(`trades/${tradeId}`)
}

export async function updateTradeNotes(tradeId: string, notes: string | null): Promise<JournalTrade> {
    return fetchCrud<JournalTrade>(`trades/${tradeId}/notes`, {
        method: 'PATCH', body: JSON.stringify({ notes }),
    })
}

export async function addTradeTags(tradeId: string, tagIds: string[]): Promise<JournalTag[]> {
    return fetchCrud<JournalTag[]>(`trades/${tradeId}/tags`, {
        method: 'POST', body: JSON.stringify({ tag_ids: tagIds }),
    })
}

export async function removeTradeTag(tradeId: string, tagId: string): Promise<void> {
    await fetchCrud<{ deleted: boolean }>(`trades/${tradeId}/tags/${tagId}`, { method: 'DELETE' })
}

export interface ActivePosition {
    id: string; symbol: string; side: string; status: string
    entry_price: string; entry_quantity: string
    stop_loss_price: string | null
    take_profit_targets: { price: string; percentage: number; status: string }[] | null
    created_at: string; exchange_account_id: string
}

export async function fetchActivePositions(): Promise<ActivePosition[]> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/trades`)
    if (!res.ok) return []
    const data = await res.json()
    return data.data || []
}

export async function getDraftNotes(tradeGroupId: string): Promise<{ notes: string | null }> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/journal/drafts/${tradeGroupId}`)
    if (!res.ok) return { notes: null }
    return res.json()
}

export async function saveDraftNotes(tradeGroupId: string, notes: string | null): Promise<void> {
    await fetchWithCredentials(`${API_BASE}/api/v1/journal/drafts/${tradeGroupId}/notes`, {
        method: 'PATCH', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ notes }),
    })
}

export async function triggerJournalSync(): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/journal/sync`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
    })
    if (res.status === 409) throw Object.assign(new Error('Sync already running'), { code: 409 })
    if (!res.ok) throw new Error(`Sync error: ${res.status}`)
}
