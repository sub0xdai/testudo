/** @anchor api:journal:analytics
 * @tags api */

import { API_BASE, fetchWithCredentials, fetchApi, type StatsFilter } from './core'

// ─── Analytics Response Types ───

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

export interface SetupBreakdownItem {
    setup_tag: string
    trade_count: number
    total_pnl: string
    win_rate: string
    avg_r_multiple: string | null
    expectancy: string
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

export interface SymbolCount {
    symbol: string
    count: number
}

export interface FilterOptions {
    exchanges: string[]
    symbols: SymbolCount[]
}

// ─── Analytics API Functions ───

export async function fetchFilterOptions(exchange?: string): Promise<FilterOptions> {
    const params = new URLSearchParams()
    if (exchange) params.set('exchange', exchange)
    const res = await fetchWithCredentials(
        `${API_BASE}/api/v1/journal/analytics/filter-options?${params}`,
    )
    if (!res.ok) throw new Error(`API error: ${res.status}`)
    return res.json()
}

export async function fetchOverview(filters: StatsFilter): Promise<OverviewResponse> {
    return fetchApi<OverviewResponse>('overview', filters)
}

export async function fetchEquityCurve(
    filters: StatsFilter,
): Promise<{ data: EquityPoint[] }> {
    return fetchApi('equity-curve', filters)
}

export async function fetchDailyPnl(
    filters: StatsFilter,
): Promise<{ data: DailyPnlPoint[] }> {
    return fetchApi('daily-pnl', filters)
}

export async function fetchSymbolBreakdown(
    filters: StatsFilter,
): Promise<{ data: SymbolBreakdownItem[] }> {
    return fetchApi('symbol-breakdown', filters)
}

export async function fetchSetupBreakdown(
    filters: StatsFilter,
): Promise<{ data: SetupBreakdownItem[] }> {
    return fetchApi('setup-breakdown', filters)
}

export async function fetchDurationProfit(
    filters: StatsFilter,
): Promise<{ data: DurationProfitPoint[] }> {
    return fetchApi('duration-profit', filters)
}

export async function fetchReturnDistribution(
    filters: StatsFilter,
): Promise<{ data: ReturnBucket[] }> {
    return fetchApi('return-distribution', filters)
}

export async function fetchTimeDistribution(
    filters: StatsFilter,
): Promise<{ data: TimeSlot[] }> {
    return fetchApi('time-distribution', filters)
}

// ─── Batched Analytics (PERF-02) ───

export type BatchSection =
    | 'overview'
    | 'equity_curve'
    | 'daily_pnl'
    | 'symbol_breakdown'
    | 'setup_breakdown'
    | 'duration_profit'
    | 'return_distribution'
    | 'time_distribution'

export interface BatchAnalyticsResponse {
    overview?: OverviewResponse | { error: string }
    equity_curve?: { data: EquityPoint[] } | { error: string }
    daily_pnl?: { data: DailyPnlPoint[] } | { error: string }
    symbol_breakdown?: { data: SymbolBreakdownItem[] } | { error: string }
    setup_breakdown?: { data: SetupBreakdownItem[] } | { error: string }
    duration_profit?: { data: DurationProfitPoint[] } | { error: string }
    return_distribution?: { data: ReturnBucket[] } | { error: string }
    time_distribution?: { data: TimeSlot[] } | { error: string }
}

export async function fetchAnalyticsBatch(
    sections: BatchSection[] | undefined,
    filter: StatsFilter,
): Promise<BatchAnalyticsResponse> {
    const res = await fetchWithCredentials(
        `${API_BASE}/api/v1/journal/analytics/batch`,
        {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ filter, sections }),
        },
    )
    if (!res.ok) throw new Error(`API error: ${res.status}`)
    return res.json()
}
