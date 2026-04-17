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

async function fetchWithCredentials(url: string, init?: RequestInit): Promise<Response> {
  const opts: RequestInit = { ...init, credentials: 'include' }
  let res = await fetch(url, opts)
  if (res.status === 401) {
    const refreshRes = await fetch(`${API_BASE}/api/v1/auth/refresh`, {
      method: 'POST',
      credentials: 'include',
    })
    if (!refreshRes.ok) throw new Error('Session expired')
    res = await fetch(url, opts)
  }
  return res
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
  equity?: string
  peak: string
  drawdown: string
  drawdown_pct: string
  is_true_equity?: boolean
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
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/journal/analytics/filter-options?${params}`)
  if (!res.ok) throw new Error(`API error: ${res.status}`)
  return res.json()
}

async function fetchApi<T>(path: string, filters: StatsFilter): Promise<T> {
  const params = buildParams(filters)
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/journal/analytics/${path}?${params}`)
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

export interface TradeWithTags extends JournalTrade {
  tags: JournalTag[]
}

export interface TradesResponse {
  trades: TradeWithTags[]
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
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/journal/${path}`, {
    headers: { 'Content-Type': 'application/json' },
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
    method: 'PATCH',
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

// --- Image upload + storage (JNL-18) ---

export async function uploadJournalImage(file: File): Promise<{ url: string }> {
  const formData = new FormData()
  formData.append('file', file)
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/journal/upload`, {
    method: 'POST',
    body: formData,
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ message: `Upload failed: ${res.status}` }))
    throw new UploadError(
      err.message || `Upload failed: ${res.status}`,
      err.error,
      err.details,
    )
  }
  return res.json()
}

export class UploadError extends Error {
  constructor(
    message: string,
    public code?: string,
    public details?: { used_bytes?: number; quota_bytes?: number; remaining_bytes?: number },
  ) {
    super(message)
    this.name = 'UploadError'
  }
}

export interface StorageUsage {
  used_bytes: number
  quota_bytes: number
  image_count: number
}

export async function fetchStorageUsage(): Promise<StorageUsage> {
  return fetchCrud<StorageUsage>('storage')
}

export async function deleteImage(imageId: string): Promise<void> {
  await fetchCrud<{ deleted: boolean }>(`images/${imageId}`, {
    method: 'DELETE',
  })
}

// ─── Active Positions / Draft Notes API (JNL-20) ───

export interface ActivePosition {
  id: string
  symbol: string
  side: string
  status: string
  entry_price: string
  entry_quantity: string
  stop_loss_price: string | null
  take_profit_targets: { price: string; percentage: number; status: string }[] | null
  created_at: string
  exchange_account_id: string
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
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ notes }),
  })
}

// ─── Exchange Management API ───

export interface ExchangeInfo {
  id: string
  name: string
  display_name?: string
  type: string
  exchange_type?: string
  requires_passphrase?: boolean
  supported_features?: string[]
  description?: string
  required_credentials?: string[]
}

export interface ExchangeAccount {
  id: string
  exchange_name: string
  account_name: string
  is_active: boolean
  auth_mode: string
  agent_wallet_address?: string | null
  requires_reauthorization?: boolean | null
  last_used_at?: string | null
  created_at: string
}

export interface AddExchangeAccountPayload {
  exchange_name: string
  account_name: string
  api_key: string
  secret: string
  passphrase?: string
}

export interface TestConnectionResult {
  success: boolean
  latency_ms: number | null
  error?: string
}

export interface ExchangeBalanceEntry {
  asset: string
  total: string
  available: string
  used: string
}

export interface ExchangeBalanceResponse {
  balances: ExchangeBalanceEntry[]
}

export interface InitAgentWalletResponse {
  account_id: string
  agent_address: string
}

export interface ApproveDataResponse {
  typed_data: Record<string, unknown>
  nonce: number
}

export interface ApproveAgentResponse {
  success: boolean
}

export interface MigrateToAgentWalletResponse {
  success: boolean
}

export interface RevokeAgentResponse {
  success: boolean
}

async function fetchExchange<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/exchanges${path}`, {
    ...init,
    headers: { 'Content-Type': 'application/json', ...init?.headers },
  })
  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new Error(text || `Exchange API error: ${res.status}`)
  }
  return res.json()
}

export const exchangeApi = {
  listExchanges: async () => {
    const res = await fetchExchange<{ exchanges: ExchangeInfo[] }>('')
    return res.exchanges
  },
  listAccounts: () => fetchExchange<ExchangeAccount[]>('/accounts'),
  addAccount: (payload: AddExchangeAccountPayload) =>
    fetchExchange<ExchangeAccount>('/accounts', {
      method: 'POST',
      body: JSON.stringify(payload),
    }),
  deleteAccount: (id: string) =>
    fetchExchange<void>(`/accounts/${id}`, { method: 'DELETE' }),
  testConnection: (id: string) =>
    fetchExchange<TestConnectionResult>(`/accounts/${id}/test`, { method: 'POST' }),
  fetchBalance: (id: string) =>
    fetchExchange<ExchangeBalanceResponse>(`/accounts/${id}/balance`),
  initAgentWallet: (walletAddress: string) =>
    fetchExchange<InitAgentWalletResponse>('/agent-wallet/init', {
      method: 'POST',
      body: JSON.stringify({ wallet_address: walletAddress }),
    }),
  getApproveData: (accountId: string) =>
    fetchExchange<ApproveDataResponse>(`/agent-wallet/approve-data`, {
      method: 'POST',
      body: JSON.stringify({ account_id: accountId }),
    }),
  approveAgent: (accountId: string, signature: string, nonce: number) =>
    fetchExchange<ApproveAgentResponse>('/agent-wallet/approve', {
      method: 'POST',
      body: JSON.stringify({ account_id: accountId, signature, nonce }),
    }),
  migrateToAgentWallet: (accountId: string, walletAddress: string) =>
    fetchExchange<MigrateToAgentWalletResponse>('/agent-wallet/migrate', {
      method: 'POST',
      body: JSON.stringify({ account_id: accountId, wallet_address: walletAddress }),
    }),
  revokeAgent: (id: string) =>
    fetchExchange<RevokeAgentResponse>(`/agent-wallet/${id}/revoke`, { method: 'DELETE' }),
}

// ─── Risk Snapshot API (RSK-01) ───

export interface PositionEntry {
  symbol: string
  side: 'long' | 'short'
  entry_price: string
  mark_price: string
  quantity: string
  notional_usd: string
  unrealized_pnl_usd: string
}

export interface VenuePositions {
  exchange_id: string
  exchange_name: string
  positions: PositionEntry[]
}

export interface VenueMargin {
  exchange_id: string
  exchange_name: string
  free_usd: string
  used_usd: string
  total_usd: string
}

export interface CorrelationBucket {
  bucket: string
  direction: 'long' | 'short' | 'mixed'
  effective_notional_usd: string
  contributing_symbols: string[]
}

export interface RiskSnapshot {
  net_exposure_usd: string
  aggregate_leverage: string
  free_margin_usd: string
  long_pct: string
  short_pct: string
  net_delta_usd: string
  positions_by_venue: VenuePositions[]
  margin_by_venue: VenueMargin[]
  correlation_stack: CorrelationBucket[]
  as_of: string
}

export async function fetchRiskSnapshot(): Promise<RiskSnapshot> {
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/risk/snapshot`)
  if (!res.ok) throw new Error(`Risk snapshot error: ${res.status}`)
  return res.json()
}

export async function pairExtension(): Promise<{ code: string }> {
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/auth/pair-extension`, {
    method: 'POST',
    credentials: 'include',
  })
  if (!res.ok) throw new Error('Failed to generate pairing code')
  return res.json()
}

export async function checkPairStatus(): Promise<{ paired: boolean }> {
  const res = await fetchWithCredentials(`${API_BASE}/api/v1/auth/pair-status`)
  if (!res.ok) return { paired: false }
  return res.json()
}
