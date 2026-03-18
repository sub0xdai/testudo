const API_BASE = import.meta.env.VITE_API_URL || 'http://127.0.0.1:8080'

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

export async function fetchOverview(filters: StatsFilter): Promise<OverviewResponse> {
  const params = buildParams(filters)
  const res = await fetch(`${API_BASE}/api/v1/journal/analytics/overview?${params}`, {
    headers: { Authorization: `Bearer ${getToken()}` },
  })
  if (!res.ok) throw new Error(`API error: ${res.status}`)
  return res.json()
}
