import { For, Show } from 'solid-js'
import type { PositionEntry, RiskSnapshot, VenuePositions } from '../../api/client'
import { HelpTip } from '../HelpTip'
import { HELP } from '../../lib/help-content'
import { formatCurrency, formatNumber, formatPrice, pnlColor } from '../../lib/formatters'

interface PositionsByVenueProps {
  snapshot: RiskSnapshot
}

export function PositionsByVenue(props: PositionsByVenueProps) {
  const venuesWithPositions = () =>
    props.snapshot.positions_by_venue.filter((v) => v.positions.length > 0)

  const totalPositions = () =>
    props.snapshot.positions_by_venue.reduce((acc, v) => acc + v.positions.length, 0)

  const venueCount = () => props.snapshot.positions_by_venue.length

  return (
    <section aria-label="Positions by venue">
      <div class="flex items-center gap-2 mb-3 px-1">
        <span
          class={`w-2 h-2 rounded-full ${
            totalPositions() > 0 ? 'bg-signal-green animate-pulse' : 'bg-text-tertiary/40'
          }`}
        />
        <span class="font-display text-[10px] font-bold tracking-section text-text-secondary uppercase">
          Positions by Venue
        </span>
        <HelpTip text={HELP['risk.positions_by_venue']} />
        <Show when={totalPositions() > 0}>
          <span class="font-mono text-[10px] text-text-tertiary bg-text-primary/10 px-1.5 py-0.5">
            {totalPositions()}
          </span>
        </Show>
      </div>

      <Show
        when={totalPositions() > 0}
        fallback={
          <div class="border border-container-border bg-container-bg p-8 text-center">
            <span class="font-mono text-sm text-text-tertiary">
              No open positions across {venueCount()} venue{venueCount() === 1 ? '' : 's'}
            </span>
          </div>
        }
      >
        <div class="flex flex-col gap-6">
          <For each={venuesWithPositions()}>{(venue) => <VenueSection venue={venue} />}</For>
        </div>
      </Show>
    </section>
  )
}

function VenueSection(props: { venue: VenuePositions }) {
  const count = () => props.venue.positions.length
  return (
    <div class="border border-container-border bg-container-bg">
      <div class="px-4 py-3 border-b border-container-border/60 flex items-center gap-3">
        <span class="font-display text-xs font-bold tracking-wider text-text-primary uppercase">
          {props.venue.exchange_name}
        </span>
        <span class="font-mono text-[10px] text-text-tertiary">
          {count()} {count() === 1 ? 'position' : 'positions'}
        </span>
      </div>
      <div class="overflow-x-auto">
        <table class="w-full">
          <thead>
            <tr class="border-b border-container-border/30">
              <th class="px-4 py-2 text-left font-display text-[10px] tracking-section text-text-tertiary uppercase">
                Symbol
              </th>
              <th class="px-4 py-2 text-left font-display text-[10px] tracking-section text-text-tertiary uppercase">
                Side
              </th>
              <th class="px-4 py-2 text-right font-display text-[10px] tracking-section text-text-tertiary uppercase">
                Entry
              </th>
              <th class="px-4 py-2 text-right font-display text-[10px] tracking-section text-text-tertiary uppercase">
                Mark
              </th>
              <th class="px-4 py-2 text-right font-display text-[10px] tracking-section text-text-tertiary uppercase">
                Size
              </th>
              <th class="px-4 py-2 text-right font-display text-[10px] tracking-section text-text-tertiary uppercase">
                Unrealized
              </th>
            </tr>
          </thead>
          <tbody>
            <For each={props.venue.positions}>{(pos) => <PositionRow pos={pos} />}</For>
          </tbody>
        </table>
      </div>
    </div>
  )
}

function PositionRow(props: { pos: PositionEntry }) {
  const sideClass = () => (props.pos.side === 'long' ? 'text-signal-green' : 'text-signal-red')
  return (
    <tr class="border-b border-container-border/20 last:border-b-0 hover:bg-elevated/40 transition-colors">
      <td class="px-4 py-2.5 font-mono text-xs text-text-primary whitespace-nowrap">
        {props.pos.symbol}
      </td>
      <td class={`px-4 py-2.5 font-mono text-xs uppercase whitespace-nowrap ${sideClass()}`}>
        {props.pos.side}
      </td>
      <td class="px-4 py-2.5 font-mono text-xs text-text-secondary text-right whitespace-nowrap">
        {formatPrice(props.pos.entry_price)}
      </td>
      <td class="px-4 py-2.5 font-mono text-xs text-text-secondary text-right whitespace-nowrap">
        {formatPrice(props.pos.mark_price)}
      </td>
      <td class="px-4 py-2.5 font-mono text-xs text-text-secondary text-right whitespace-nowrap">
        {formatNumber(props.pos.quantity, 4)}
      </td>
      <td class={`px-4 py-2.5 font-mono text-xs text-right whitespace-nowrap ${pnlColor(props.pos.unrealized_pnl_usd)}`}>
        {formatCurrency(props.pos.unrealized_pnl_usd)}
      </td>
    </tr>
  )
}
