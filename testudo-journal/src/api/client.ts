const API_BASE = import.meta.env.VITE_API_URL || ''

export interface StatsFilter {
  exchange?: string
  symbol?: string
  dateFrom?: string
  dateTo?: string
}

export interface AccountStats {
  total_trades: number
  total_pnl: string
  total_fees: string
  net_pnl: string
}

export interface PerformanceStats {
  win_rate: string
  profit_factor: string
  avg_win: string
  avg_loss: string
  largest_win: string
  largest_loss: string
  expectancy: string
  avg_r_multiple: string
  trades_per_day: string
  avg_duration_secs: number
  total_duration_days: number
}

export interface RiskStats {
  max_drawdown: string
  max_drawdown_pct: string
  worst_day: string
  worst_week: string
  worst_month: string
  best_day: string
  best_week: string
  best_month: string
  risk_of_ruin: string
  current_streak: number
  best_streak: number
  worst_streak: number
}

export interface OverviewResponse {
  account: AccountStats
  performance: PerformanceStats
  risk: RiskStats
}

function getToken(): string {
  return localStorage.getItem('testudo_token') ?? ''
}

function buildParams(filters: StatsFilter): URLSearchParams {
  const params = new URLSearchParams()
  if (filters.exchange) params.set('exchange', filters.exchange)
  if (filters.symbol) params.set('symbol', filters.symbol)
  if (filters.dateFrom) params.set('date_from', filters.dateFrom)
  if (filters.dateTo) params.set('date_to', filters.dateTo)
  return params
}

export interface EquityPoint {
  date: string
  cumulative_pnl: string
  peak: string
  drawdown: string
  drawdown_pct: string
}

export interface DailyPnlPoint {
  date: string
  pnl: string
  trade_count: number
}

export interface SymbolBreakdownItem {
  symbol: string
  trade_count: number
  total_pnl: string
  win_rate: string
}

export interface DurationProfitPoint {
  duration_secs: number
  pnl: string
  symbol: string
}

export interface ReturnBucket {
  bucket: string
  count: number
}

export interface TimeSlot {
  day_of_week: number
  hour: number
  trade_count: number
  avg_pnl: string
}

// --- Filter options (UXP-09) ---

export interface SymbolCount {
  symbol: string
  count: number
}

export interface FilterOptions {
  exchanges: string[]
  symbols: SymbolCount[]
}

export async function fetchFilterOptions(exchange?: string): Promise<FilterOptions> {
  const params = new URLSearchParams()
  if (exchange) params.set('exchange', exchange)
  const res = await fetch(`${API_BASE}/api/v1/journal/analytics/filter-options?${params}`, {
    headers: { Authorization: `Bearer ${getToken()}` },
  })
  if (!res.ok) throw new Error(`API error: ${res.status}`)
  return res.json()
}

async function fetchApi<T>(path: string, filters: StatsFilter): Promise<T> {
  const params = buildParams(filters)
  const res = await fetch(`${API_BASE}/api/v1/journal/analytics/${path}?${params}`, {
    headers: { Authorization: `Bearer ${getToken()}` },
  })
  if (!res.ok) throw new Error(`API error: ${res.status}`)
  return res.json()
}

export async function fetchOverview(filters: StatsFilter): Promise<OverviewResponse> {
  return fetchApi<OverviewResponse>('overview', filters)
}

export async function fetchEquityCurve(filters: StatsFilter): Promise<{ data: EquityPoint[] }> {
  return fetchApi('equity-curve', filters)
}

export async function fetchDailyPnl(filters: StatsFilter): Promise<{ data: DailyPnlPoint[] }> {
  return fetchApi('daily-pnl', filters)
}

export async function fetchSymbolBreakdown(filters: StatsFilter): Promise<{ data: SymbolBreakdownItem[] }> {
  return fetchApi('symbol-breakdown', filters)
}

export async function fetchDurationProfit(filters: StatsFilter): Promise<{ data: DurationProfitPoint[] }> {
  return fetchApi('duration-profit', filters)
}

export async function fetchReturnDistribution(filters: StatsFilter): Promise<{ data: ReturnBucket[] }> {
  return fetchApi('return-distribution', filters)
}

export async function fetchTimeDistribution(filters: StatsFilter): Promise<{ data: TimeSlot[] }> {
  return fetchApi('time-distribution', filters)
}

// --- Trade CRUD API ---

export interface JournalTrade {
  id: string
  user_id: string
  exchange: string
  symbol: string
  side: string
  entry_price: string
  exit_price: string
  quantity: string
  leverage: number
  realized_pnl: string
  realized_pnl_pct: string
  fees: string
  net_pnl: string
  stop_price: string | null
  target_price: string | null
  risk_amount: string | null
  r_multiple: string | null
  opened_at: string
  closed_at: string
  duration_secs: number
  trade_group_id: string | null
  notes: string | null
  created_at: string
  updated_at: string
}

export interface JournalTag {
  id: string
  user_id: string
  name: string
  color: string | null
}

export interface JournalEntry {
  id: string
  user_id: string
  trade_id: string | null
  entry_date: string | null
  title: string
  body: string
  entry_type: string
  created_at: string
  updated_at: string
}

export interface TradeDetail extends JournalTrade {
  entries: JournalEntry[]
  tags: JournalTag[]
}

export interface TradesResponse {
  trades: JournalTrade[]
  total: number
  page: number
  limit: number
}

export interface TradeListParams {
  page?: number
  limit?: number
  exchange?: string
  symbol?: string
  side?: string
  tag?: string
  dateFrom?: string
  dateTo?: string
  sort?: string
  order?: string
}

async function fetchCrud<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}/api/v1/journal/${path}`, {
    headers: {
      Authorization: `Bearer ${getToken()}`,
      'Content-Type': 'application/json',
    },
    ...init,
  })
  if (!res.ok) throw new Error(`API error: ${res.status}`)
  return res.json()
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
    method: 'PUT',
    body: JSON.stringify({ notes }),
  })
}

export async function addTradeTags(tradeId: string, tagIds: string[]): Promise<JournalTag[]> {
  return fetchCrud<JournalTag[]>(`trades/${tradeId}/tags`, {
    method: 'POST',
    body: JSON.stringify({ tag_ids: tagIds }),
  })
}

export async function removeTradeTag(tradeId: string, tagId: string): Promise<void> {
  await fetchCrud<{ deleted: boolean }>(`trades/${tradeId}/tags/${tagId}`, {
    method: 'DELETE',
  })
}

export async function fetchTags(): Promise<JournalTag[]> {
  return fetchCrud<JournalTag[]>('tags')
}

export async function fetchEntries(params: { tradeId?: string; page?: number; limit?: number }): Promise<{ entries: JournalEntry[]; total: number }> {
  const p = new URLSearchParams()
  if (params.tradeId) p.set('trade_id', params.tradeId)
  if (params.page) p.set('page', String(params.page))
  if (params.limit) p.set('limit', String(params.limit))
  return fetchCrud(`entries?${p}`)
}

export async function createEntry(data: {
  title: string
  body: string
  entry_type?: string
  trade_id?: string
  entry_date?: string
}): Promise<JournalEntry> {
  return fetchCrud<JournalEntry>('entries', {
    method: 'POST',
    body: JSON.stringify(data),
  })
}

export async function updateEntry(entryId: string, data: {
  title: string
  body: string
  entry_type?: string
}): Promise<JournalEntry> {
  return fetchCrud<JournalEntry>(`entries/${entryId}`, {
    method: 'PUT',
    body: JSON.stringify(data),
  })
}

export async function deleteEntry(entryId: string): Promise<void> {
  await fetchCrud<{ deleted: boolean }>(`entries/${entryId}`, {
    method: 'DELETE',
  })
}

export async function createTag(data: { name: string; color?: string }): Promise<JournalTag> {
  return fetchCrud<JournalTag>('tags', {
    method: 'POST',
    body: JSON.stringify(data),
  })
}

export async function updateTag(tagId: string, data: { name?: string; color?: string }): Promise<JournalTag> {
  return fetchCrud<JournalTag>(`tags/${tagId}`, {
    method: 'PUT',
    body: JSON.stringify(data),
  })
}

export async function deleteTag(tagId: string): Promise<void> {
  await fetchCrud<{ deleted: boolean }>(`tags/${tagId}`, {
    method: 'DELETE',
  })
}

// --- Image upload ---

export async function uploadJournalImage(file: File): Promise<{ url: string }> {
  const formData = new FormData()
  formData.append('file', file)
  const res = await fetch(`${API_BASE}/api/v1/journal/upload`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${getToken()}` },
    body: formData,
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ message: `Upload failed: ${res.status}` }))
    throw new Error(err.message || `Upload failed: ${res.status}`)
  }
  return res.json()
}
