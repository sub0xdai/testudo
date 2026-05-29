/** @anchor api:journal:coach
 * @tags api */

import { fetchWithCredentials, API_BASE } from './core'

export type CoachPatternKind =
    | 'sizing_drift' | 'frequency_spike' | 'session_anomaly'
    | 'setup_fatigue' | 'correlation_stack' | 'streak_risk'

export type CoachSeverity = 'info' | 'notable' | 'concerning'

export interface CoachSetupBaseline {
    trade_count: number; avg_r_multiple: string; win_rate: string
}

export interface CoachUserBaseline {
    avg_trades_per_day: string; avg_position_size_usd: string
    typical_session_hours_utc: number[]; win_rate: string; avg_r_multiple: string
    p90_trades_per_6h: string; setup_baselines: Record<string, CoachSetupBaseline>
}

export interface CoachWeekStats {
    trade_count: number; win_rate: string; total_pnl: string; total_r: string
    trades_by_hour_utc: number[]; by_setup: Record<string, CoachSetupBaseline>
}

export interface CoachTradeEvidence {
    id: string; short_id: string; symbol: string; side: string
    opened_at: string; closed_at: string; pnl: string
    r_multiple: string | null; setup_tag: string | null; position_size_usd: string
}

export interface CoachFlaggedPattern {
    pattern: CoachPatternKind; severity: CoachSeverity
    evidence: string[]; metrics: Record<string, unknown>
}

export interface CoachDigest {
    user_id: string; week_start: string; week_end: string
    baseline: CoachUserBaseline; week_stats: CoachWeekStats
    flagged_patterns: CoachFlaggedPattern[]; flagged_trades: CoachTradeEvidence[]
}

export interface CoachNarrativeSection {
    pattern: CoachPatternKind; body: string; citations: string[]
}

export interface StoredCoachReport {
    id: string; user_id: string; week_start: string; week_end: string
    generated_at: string; model_used: string; headline: string | null
    narrative_sections: CoachNarrativeSection[] | null
    digest: CoachDigest; cache_hit_ratio: string | null
    banner_dismissed_at: string | null
}

export interface CoachLatestResponse { data: StoredCoachReport | null; has_new_indicator: boolean }
export interface CoachArchiveResponse { data: StoredCoachReport[] }
export interface CoachPreferenceResponse { coach_enabled: boolean }

export async function fetchLatestCoachReport(): Promise<CoachLatestResponse> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/coach/latest`)
    if (!res.ok) throw new Error(`Coach latest error: ${res.status}`)
    return res.json()
}

export async function fetchCoachArchive(limit = 20, offset = 0): Promise<CoachArchiveResponse> {
    const params = new URLSearchParams()
    params.set('limit', String(limit)); params.set('offset', String(offset))
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/coach/archive?${params}`)
    if (!res.ok) throw new Error(`Coach archive error: ${res.status}`)
    return res.json()
}

export async function fetchCoachPreference(): Promise<CoachPreferenceResponse> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/coach/preference`)
    if (!res.ok) throw new Error(`Coach preference error: ${res.status}`)
    return res.json()
}

export async function setCoachPreference(enabled: boolean): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/coach/preference`, {
        method: 'PATCH', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled }),
    })
    if (!res.ok) throw new Error(`Coach preference update error: ${res.status}`)
}

export async function markCoachViewed(): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/coach/mark-viewed`, { method: 'POST' })
    if (!res.ok) throw new Error(`Coach mark-viewed error: ${res.status}`)
}

export async function dismissCoachBanner(reportId: string): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/coach/${reportId}/dismiss-banner`, { method: 'PATCH' })
    if (!res.ok) throw new Error(`Coach dismiss-banner error: ${res.status}`)
}
