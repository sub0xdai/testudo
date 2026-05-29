/** @anchor api:journal:types
 * @tags api */

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
    pseudocount_k: number
    p_setup_raw: number
    p_global_raw: number
    computed_at: string
}

export interface SetupTagEntry {
    name: string
    last_used: string
    uses: number
}

export interface JournalTrade {
    id: string; user_id: string; exchange: string; symbol: string
    side: string; entry_price: string; exit_price: string
    quantity: string; leverage: number
    realized_pnl: string; realized_pnl_pct: string
    fees: string; net_pnl: string
    stop_price: string | null; target_price: string | null
    risk_amount: string | null; r_multiple: string | null
    opened_at: string; closed_at: string; duration_secs: number
    trade_group_id: string | null; notes: string | null
    setup_tag: string | null; kelly_inputs: KellyInputs | null
    created_at: string; updated_at: string
}

export interface JournalTag {
    id: string; user_id: string; name: string; color: string | null
}

export interface JournalEntry {
    id: string; user_id: string; trade_id: string | null
    entry_date: string | null; title: string; body: string
    entry_type: string; created_at: string; updated_at: string
}
