/** @anchor api:journal:types
 * @tags api */

// Re-export from core.ts for backward compatibility
export type { StatsFilter } from './core'

export type KellyInputs = {
    mode: 'calibrated_kelly'
    baseline_risk_pct: number
    effective_risk_pct: number
    edge_multiplier: number
    p_eff: number
    avg_r_win: number
    avg_r_loss: number
    quarter_kelly: number
    n_setup: number
    n_global: number
}

export interface SetupTagEntry {
    name: string
    last_used: string
    uses: number
}
