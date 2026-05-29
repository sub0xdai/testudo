/** @anchor api:journal:risk
 * @tags api */

import { fetchWithCredentials, API_BASE } from './core'

export interface PositionEntry {
    symbol: string; side: 'long' | 'short'; entry_price: string
    mark_price: string; quantity: string; notional_usd: string
    unrealized_pnl_usd: string; leverage?: string
}

export interface VenuePositions {
    exchange_id: string; exchange_name: string; positions: PositionEntry[]
}

export interface VenueMargin {
    exchange_id: string; exchange_name: string
    free_usd: string; used_usd: string; total_usd: string
}

export interface CorrelationBucket {
    bucket: string; direction: 'long' | 'short' | 'mixed'
    effective_notional_usd: string; contributing_symbols: string[]
}

export interface RiskSnapshot {
    net_exposure_usd: string; aggregate_leverage: string; free_margin_usd: string
    long_pct: string; short_pct: string; net_delta_usd: string
    positions_by_venue: VenuePositions[]; margin_by_venue: VenueMargin[]
    correlation_stack: CorrelationBucket[]; as_of: string
}

export async function fetchRiskSnapshot(): Promise<RiskSnapshot> {
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/risk/snapshot`)
    if (!res.ok) throw new Error(`Risk snapshot error: ${res.status}`)
    return res.json()
}
