/** @anchor api:journal:dignitas
 * @tags api */

import { fetchWithCredentials, API_BASE } from './core'

export interface DignitasInputContributions {
    drawdown_adherence: string; risk_per_trade_consistency: string
    setup_adherence: string; coach_severity_penalty: string
    journal_consistency: string
}

export interface DignitasStreak { days_clean: number; longest_ever: number }

export interface DignitasCurrent {
    score: string; delta_7d: string | null; cold_start: boolean
    trade_count_30d: number; pill_hidden: boolean
    contributions: DignitasInputContributions; streak: DignitasStreak | null
}

export interface DignitasHistoryPoint { date: string; score: string; cold_start: boolean }
export interface DignitasHistory { snapshots: DignitasHistoryPoint[] }

export async function fetchDignitasMe(): Promise<DignitasCurrent> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/me`)
    if (!res.ok) throw new Error(`Dignitas /me error: ${res.status}`)
    return res.json()
}

export async function fetchDignitasHistory(days = 90): Promise<DignitasHistory> {
    const params = new URLSearchParams(); params.set('days', String(days))
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/history?${params}`)
    if (!res.ok) throw new Error(`Dignitas history error: ${res.status}`)
    return res.json()
}

export async function patchDignitasPreference(pillHidden: boolean): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/preferences`, {
        method: 'PATCH', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pill_hidden: pillHidden }),
    })
    if (!res.ok) throw new Error(`Dignitas preferences error: ${res.status}`)
}

// ─── Identity / Public Profile ───

export interface IdentityPreferences {
    handle: string | null; bio: string | null
    show_score: boolean; show_sparkline: boolean; show_streak: boolean
    allow_indexing: boolean; can_change_handle_at: string | null
}

export interface PublicProfile {
    handle: string; bio: string | null; score: string | null
    sparkline: { date: string; score: string }[] | null
    streak_days: number | null; longest_ever: number | null
    member_since: string; allow_indexing: boolean
}

export async function fetchIdentity(): Promise<IdentityPreferences> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/identity`)
    if (!res.ok) throw new Error(`Identity error: ${res.status}`)
    return res.json()
}

export async function claimHandle(handle: string, bio?: string): Promise<IdentityPreferences> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/handle`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ handle, bio }),
    })
    if (!res.ok) {
        const err = await res.json().catch(() => ({}))
        throw Object.assign(new Error(err.message || `Claim handle error: ${res.status}`), { code: err.code, status: res.status, data: err })
    }
    return res.json()
}

export async function releaseHandle(): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/handle`, { method: 'DELETE' })
    if (!res.ok) {
        const err = await res.json().catch(() => ({}))
        throw Object.assign(new Error(err.message || `Release handle error: ${res.status}`), { code: err.code, status: res.status, data: err })
    }
}

export async function patchVisibility(patch: {
    show_score?: boolean; show_sparkline?: boolean; show_streak?: boolean
}): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/visibility`, {
        method: 'PATCH', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(patch),
    })
    if (!res.ok) throw new Error(`Visibility patch error: ${res.status}`)
}

export async function patchIndexing(allowIndexing: boolean): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/visibility`, {
        method: 'PATCH', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ allow_indexing: allowIndexing }),
    })
    if (!res.ok) throw new Error(`Indexing patch error: ${res.status}`)
}

export async function updateBio(bio: string | null): Promise<void> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/dignitas/handle`, {
        method: 'PATCH', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ bio }),
    })
    if (!res.ok) {
        const err = await res.json().catch(() => ({}))
        throw Object.assign(new Error(err.message || `Bio update error: ${res.status}`), { code: err.code, status: res.status })
    }
}

export async function fetchPublicProfile(handle: string): Promise<PublicProfile | null> {
    const res = await fetch(`${API_BASE}/api/v1/public/profile/${encodeURIComponent(handle)}`)
    if (res.status === 404) return null
    if (!res.ok) throw new Error(`Public profile error: ${res.status}`)
    return res.json()
}

// ─── Auth pairing ───

export async function pairExtension(): Promise<{ code: string }> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/auth/pair-extension`, {
        method: 'POST', credentials: 'include',
    })
    if (!res.ok) throw new Error('Failed to generate pairing code')
    return res.json()
}

export async function checkPairStatus(): Promise<{ paired: boolean }> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/auth/pair-status`)
    if (!res.ok) return { paired: false }
    return res.json()
}
