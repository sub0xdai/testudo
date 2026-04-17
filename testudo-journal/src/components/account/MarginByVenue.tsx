import { For, Show } from 'solid-js'
import type { RiskSnapshot, VenueMargin } from '../../api/client'
import { HelpTip } from '../HelpTip'
import { HELP } from '../../lib/help-content'
import { formatCurrency } from '../../lib/formatters'

interface MarginByVenueProps {
  snapshot: RiskSnapshot
}

function stripSign(s: string): string {
  return s.replace(/^\+/, '')
}

export function MarginByVenue(props: MarginByVenueProps) {
  const sortedVenues = (): VenueMargin[] => {
    return [...props.snapshot.margin_by_venue].sort(
      (a, b) => parseFloat(b.free_usd) - parseFloat(a.free_usd),
    )
  }

  const totalFree = () =>
    props.snapshot.margin_by_venue.reduce((acc, m) => acc + parseFloat(m.free_usd), 0)

  return (
    <section aria-label="Margin by venue" class="border border-container-border bg-container-bg">
      <div class="flex items-center gap-2 px-4 py-3 border-b border-container-border/60">
        <span class="font-display text-[10px] font-bold tracking-section text-text-secondary uppercase">
          Margin by Venue
        </span>
        <HelpTip text={HELP['risk.margin_by_venue']} />
        <span class="flex-1" />
        <span class="font-mono text-[10px] text-text-tertiary">
          {stripSign(formatCurrency(totalFree()))} free
        </span>
      </div>

      <Show
        when={sortedVenues().length > 0}
        fallback={
          <div class="px-4 py-6 text-center">
            <span class="font-mono text-sm text-text-tertiary">No venues connected</span>
          </div>
        }
      >
        <div class="py-2">
          <For each={sortedVenues()}>{(venue) => <MarginRow venue={venue} />}</For>
        </div>
      </Show>
    </section>
  )
}

function MarginRow(props: { venue: VenueMargin }) {
  const free = () => stripSign(formatCurrency(props.venue.free_usd))
  const used = () => stripSign(formatCurrency(props.venue.used_usd))
  const total = () => stripSign(formatCurrency(props.venue.total_usd))

  return (
    <div class="px-4 py-3">
      <div class="flex items-center gap-2">
        <span class="font-display text-xs font-bold tracking-wider text-text-primary uppercase">
          {props.venue.exchange_name}
        </span>
        <span class="flex-1 border-b border-dotted border-container-border" />
        <span class="font-mono text-xs font-bold text-text-primary">{free()}</span>
      </div>
      <div class="flex items-center gap-3 mt-1 pl-1">
        <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">
          used {used()}
        </span>
        <span class="font-mono text-[10px] text-text-tertiary">·</span>
        <span class="font-mono text-[10px] text-text-tertiary uppercase tracking-wider">
          total {total()}
        </span>
      </div>
    </div>
  )
}
