import { onCleanup, onMount } from 'solid-js'
import type { KellyInputs } from '../../api/client'
import { HelpTip } from '../HelpTip'
import { HELP } from '../../lib/help-content'

function narrativeSummary(k: KellyInputs): string {
  const m = k.edge_multiplier
  const x = m.toFixed(2) + '×'
  if (m >= 2.0) {
    return `Sized up 2.0× (ceiling hit) — this setup's edge is strong enough that the clamp engaged.`
  }
  if (m > 1.05) {
    return `Sized up ${x} because this setup's ${k.n_setup}-trade history beats your ${k.n_global}-trade baseline.`
  }
  if (m <= 0.25) {
    return `Sized down 0.25× (floor hit) — calibration is weak for this setup.`
  }
  if (m < 0.95) {
    return `Sized down ${x} because this setup's ${k.n_setup}-trade history trails your baseline.`
  }
  return `Sized at baseline — calibration is neutral for this setup.`
}

function pct(v: number) {
  return (v * 100).toFixed(1) + '%'
}

function fmt(v: number, decimals = 4) {
  return v.toFixed(decimals)
}

function formatComputedAt(iso: string): string {
  try {
    return new Date(iso).toISOString().replace('T', ' ').slice(0, 19) + ' UTC'
  } catch {
    return iso
  }
}

interface RowProps {
  label: string
  value: string
  helpKey?: string
  dim?: boolean
}

function Row(props: RowProps) {
  return (
    <div class={`flex items-baseline justify-between py-1.5 border-b border-container-border/20 ${props.dim ? 'opacity-60' : ''}`}>
      <span class="text-[11px] font-mono text-text-tertiary flex items-center gap-1">
        {props.label}
        {props.helpKey && HELP[props.helpKey] && (
          <HelpTip text={HELP[props.helpKey]} position="right" />
        )}
      </span>
      <span class="text-[11px] font-mono text-text-primary tabular-nums">{props.value}</span>
    </div>
  )
}

export function KellyInputsModal(props: {
  inputs: KellyInputs
  onClose: () => void
}) {
  const k = props.inputs
  const narrative = narrativeSummary(k)

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape') props.onClose()
  }

  onMount(() => {
    document.addEventListener('keydown', handleKeyDown)
  })
  onCleanup(() => {
    document.removeEventListener('keydown', handleKeyDown)
  })

  return (
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-main-bg/80 backdrop-blur-sm"
      onClick={(e) => { if (e.target === e.currentTarget) props.onClose() }}
    >
      <div class="relative border border-signal-green/30 bg-container-bg w-full max-w-sm mx-4 shadow-2xl">
        {/* Header */}
        <div class="flex items-center justify-between px-4 pt-3 pb-2 border-b border-container-border/40">
          <div class="flex items-center gap-2">
            <span class="text-signal-green text-sm">⚡</span>
            <span class="font-mono text-[10px] tracking-widest text-text-tertiary uppercase">Kelly Sizing Inputs</span>
            <HelpTip text={HELP['kelly.badge']} position="below" />
          </div>
          <button
            onClick={props.onClose}
            class="text-text-tertiary hover:text-text-primary font-mono text-lg leading-none transition-colors"
            aria-label="Close"
          >
            &times;
          </button>
        </div>

        {/* Narrative summary */}
        <div class="px-4 py-3 bg-signal-green/5 border-b border-signal-green/20">
          <p class="text-[11px] font-mono text-signal-green/80 leading-relaxed">{narrative}</p>
        </div>

        {/* Fields */}
        <div class="px-4 py-2">
          <div class="text-[9px] font-mono tracking-widest text-text-tertiary uppercase mt-1 mb-1">Sizing</div>
          <Row label="Baseline Risk" value={pct(k.baseline_risk_pct)} />
          <Row
            label="Effective Risk"
            value={pct(k.effective_risk_pct)}
          />
          <Row
            label="Edge Multiplier"
            value={k.edge_multiplier.toFixed(4) + '×'}
            helpKey="kelly.edge_multiplier"
          />
          <Row label="Quarter Kelly" value={fmt(k.quarter_kelly, 4)} />

          <div class="text-[9px] font-mono tracking-widest text-text-tertiary uppercase mt-3 mb-1">Sample Sizes</div>
          <Row
            label="Setup Trades (n)"
            value={String(k.n_setup)}
            helpKey="kelly.n_setup"
          />
          <Row
            label="Global Trades (n)"
            value={String(k.n_global)}
            helpKey="kelly.n_global"
          />
          <Row label="Pseudocount K" value={String(k.pseudocount_k)} dim />

          <div class="text-[9px] font-mono tracking-widest text-text-tertiary uppercase mt-3 mb-1">Blended Stats</div>
          <Row
            label="Effective Win Rate (p_eff)"
            value={pct(k.p_eff)}
            helpKey="kelly.p_eff"
          />
          <Row label="Avg R Win" value={'+' + fmt(k.avg_r_win, 3)} />
          <Row label="Avg R Loss" value={'-' + fmt(k.avg_r_loss, 3)} />

          <div class="text-[9px] font-mono tracking-widest text-text-tertiary uppercase mt-3 mb-1">Raw Priors</div>
          <Row label="Setup Win Rate (raw)" value={pct(k.p_setup_raw)} dim />
          <Row label="Global Win Rate (raw)" value={pct(k.p_global_raw)} dim />

          <div class="text-[9px] font-mono tracking-widest text-text-tertiary uppercase mt-3 mb-1">Meta</div>
          <Row label="Computed At" value={formatComputedAt(k.computed_at)} dim />
        </div>

        <div class="px-4 pb-3" />
      </div>
    </div>
  )
}
