/** @anchor ui:journal:CoachBanner
 * @tags ui */

import { createResource, Show } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import { fetchLatestCoachReport, dismissCoachBanner, type StoredCoachReport } from '../../api/client'

function headlineFor(report: StoredCoachReport): string {
  if (report.headline && report.headline.trim().length > 0) return report.headline
  const first = report.digest.flagged_patterns[0]
  if (first) return `${first.pattern.replace(/_/g, ' ')} flagged this week`
  return 'Weekly coach report available'
}

export function CoachBanner() {
  const navigate = useNavigate()
  const [latest, { mutate }] = createResource(fetchLatestCoachReport)

  async function handleDismiss(e: MouseEvent, reportId: string) {
    e.stopPropagation()
    mutate({ data: null, has_new_indicator: false })
    try {
      await dismissCoachBanner(reportId)
    } catch {
      // If dismiss fails the banner will reappear on next refresh — acceptable.
    }
  }

  function handleOpen() {
    navigate('/coach')
  }

  return (
    <Show when={latest()?.data && !latest()!.data!.banner_dismissed_at}>
      {(_) => {
        const report = latest()!.data!
        const isNew = latest()!.has_new_indicator
        return (
          <div
            role="button"
            tabindex="0"
            onClick={handleOpen}
            onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleOpen() }}
            class="flex items-center justify-between gap-4 border border-container-border bg-main-bg/95 px-5 py-4 cursor-pointer hover:border-text-primary transition-colors"
          >
            <div class="flex items-center gap-3 min-w-0">
              <span
                class="inline-block w-2 h-2 rounded-full shrink-0"
                classList={{
                  'bg-signal-green animate-pulse': isNew,
                  'bg-text-tertiary': !isNew,
                }}
                aria-label={isNew ? 'new coach report' : 'coach report'}
              />
              <div class="min-w-0">
                <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-1">
                  // COACH_REPORT
                </div>
                <div class="font-mono text-sm text-text-primary truncate">
                  {headlineFor(report)}
                </div>
              </div>
            </div>
            <div class="flex items-center gap-4 shrink-0">
              <span class="font-mono text-[10px] tracking-wider text-text-secondary hidden md:inline">
                view coach report &rarr;
              </span>
              <button
                onClick={(e) => handleDismiss(e, report.id)}
                class="font-mono text-[10px] tracking-wider text-text-tertiary hover:text-signal-red transition-colors"
                aria-label="Dismiss coach banner"
              >
                DISMISS
              </button>
            </div>
          </div>
        )
      }}
    </Show>
  )
}
