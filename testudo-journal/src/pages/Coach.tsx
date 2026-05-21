import { createResource, createSignal, createEffect, Show, onMount } from 'solid-js'
import { useCachedResource, cacheKeyForSection } from '../lib/cache'
import {
  fetchLatestCoachReport,
  fetchCoachArchive,
  fetchCoachPreference,
  setCoachPreference,
  markCoachViewed,
  fetchOverview,
  type StoredCoachReport,
} from '../api/client'
import { CoachReport } from '../components/coach/CoachReport'
import { CoachArchive } from '../components/coach/CoachArchive'
import { HelpTip } from '../components/HelpTip'
import { HELP } from '../lib/help-content'
import { useAuth } from '../context/AuthContext'

const UNLOCK_THRESHOLD = 30
const ARCHIVE_PAGE_SIZE = 20

export default function Coach() {
  const [latest, { refetch: refetchLatest }] = createResource(fetchLatestCoachReport)
  const [preference, { mutate: setPreference }] = createResource(fetchCoachPreference)
  const auth = useAuth()
  const overview = useCachedResource(
    () => cacheKeyForSection('overview', {}),
    () => fetchOverview({}),
    { staleMs: 30_000, persist: true, identity: auth.user()?.id ?? null },
  )

  const [archiveOffset, setArchiveOffset] = createSignal(0)
  const [archiveItems, setArchiveItems] = createSignal<StoredCoachReport[]>([])
  const [archiveLoading, setArchiveLoading] = createSignal(false)
  const [archiveFullyLoaded, setArchiveFullyLoaded] = createSignal(false)
  const [archiveError, setArchiveError] = createSignal<string | null>(null)
  const [toggleError, setToggleError] = createSignal<string | null>(null)

  onMount(() => {
    loadMoreArchive()
  })

  // Clear the banner's ●new indicator as soon as the latest report is visible on this page.
  createEffect(() => {
    const l = latest()
    if (l?.data && l.has_new_indicator) {
      markCoachViewed().catch(() => {})
    }
  })

  async function loadMoreArchive() {
    if (archiveLoading() || archiveFullyLoaded()) return
    setArchiveLoading(true)
    setArchiveError(null)
    try {
      const res = await fetchCoachArchive(ARCHIVE_PAGE_SIZE, archiveOffset())
      const incoming = res.data ?? []
      const latestId = latest()?.data?.id
      const filtered = latestId ? incoming.filter((r) => r.id !== latestId) : incoming
      setArchiveItems([...archiveItems(), ...filtered])
      setArchiveOffset(archiveOffset() + incoming.length)
      if (incoming.length < ARCHIVE_PAGE_SIZE) setArchiveFullyLoaded(true)
    } catch (err) {
      setArchiveError((err as Error).message ?? 'Failed to load archive')
    } finally {
      setArchiveLoading(false)
    }
  }

  async function togglePreference() {
    const current = preference()?.coach_enabled ?? true
    const next = !current
    setPreference({ coach_enabled: next })
    setToggleError(null)
    try {
      await setCoachPreference(next)
      if (next) refetchLatest()
    } catch (err) {
      setPreference({ coach_enabled: current })
      setToggleError((err as Error).message ?? 'Failed to update preference')
    }
  }

  const coachEnabled = () => preference()?.coach_enabled ?? true
  const lifetimeTrades = () => overview()?.account.total_trades ?? 0
  const belowThreshold = () => !overview.loading && lifetimeTrades() < UNLOCK_THRESHOLD
  const progressPct = () =>
    Math.min(100, Math.round((lifetimeTrades() / UNLOCK_THRESHOLD) * 100))

  return (
    <div class="flex flex-col h-full overflow-y-auto">
      <div class="px-8 py-5 shrink-0 border-b border-container-border/50 bg-container-bg flex items-center gap-4">
        <h1 class="font-display text-lg font-bold tracking-wider">
          COACH
          <HelpTip text={HELP['page.coach'] ?? ''} position="below" />
        </h1>
        <span class="flex-1" />
        <button
          onClick={togglePreference}
          disabled={preference.loading}
          class="font-mono text-xs tracking-wider px-3 py-1.5 border transition-colors disabled:opacity-50"
          classList={{
            'border-text-primary text-text-primary': coachEnabled(),
            'border-container-border text-text-tertiary hover:text-text-primary': !coachEnabled(),
          }}
          aria-pressed={coachEnabled()}
        >
          {coachEnabled() ? 'COACH ON' : 'COACH OFF'}
        </button>
      </div>

      <div class="flex-1 min-h-0 max-w-5xl mx-auto w-full px-8 py-8 flex flex-col gap-10">
        <Show when={toggleError()}>
          <div class="border border-signal-red/60 bg-container-bg px-4 py-3 font-mono text-xs text-signal-red">
            {toggleError()}
          </div>
        </Show>

        {/* Opt-out state */}
        <Show when={!coachEnabled() && !preference.loading}>
          <section class="border border-container-border bg-container-bg px-6 py-8">
            <p class="font-mono text-xs tracking-wider text-text-tertiary uppercase mb-3">
              // COACH DISABLED
            </p>
            <p class="font-display text-sm text-text-secondary leading-relaxed">
              The coach is off. No weekly reports will be generated and no data will be sent to the
              narration provider. Re-enable at any time with the toggle above.
            </p>
          </section>
        </Show>

        {/* Below-threshold state */}
        <Show when={coachEnabled() && belowThreshold()}>
          <section class="border border-container-border bg-container-bg px-6 py-8">
            <p class="font-mono text-xs tracking-wider text-text-tertiary uppercase mb-3">
              // COACH LOCKED
            </p>
            <p class="font-display text-base text-text-primary leading-snug mb-4">
              {lifetimeTrades()} / {UNLOCK_THRESHOLD} trades to unlock the coach
            </p>
            <div class="h-1.5 bg-text-primary/5 w-full mb-3">
              <div
                class="h-full bg-signal-green transition-all"
                style={{ width: `${progressPct()}%` }}
              />
            </div>
            <p class="font-display text-sm text-text-secondary leading-relaxed">
              Once you have recorded {UNLOCK_THRESHOLD}+ closed trades, weekly behavioral reports
              will land here each Sunday.
            </p>
          </section>
        </Show>

        {/* Active state */}
        <Show when={coachEnabled() && !belowThreshold()}>
          <Show
            when={!latest.loading}
            fallback={<p class="font-mono text-xs text-text-tertiary">Loading latest report…</p>}
          >
            <Show
              when={latest()?.data}
              fallback={
                <section class="border border-container-border bg-container-bg px-6 py-8">
                  <p class="font-mono text-xs tracking-wider text-text-tertiary uppercase mb-3">
                    // NO REPORT YET
                  </p>
                  <p class="font-display text-sm text-text-secondary leading-relaxed">
                    No weekly report has been generated yet. Reports are produced every Sunday at
                    18:00 UTC. Weeks with fewer than 3 trades are skipped — the coach won't send
                    form letters.
                  </p>
                </section>
              }
            >
              {(report) => <CoachReport report={report()} />}
            </Show>
          </Show>

          <CoachArchive
            items={archiveItems()}
            onLoadMore={loadMoreArchive}
            canLoadMore={!archiveFullyLoaded()}
            loading={archiveLoading()}
          />
          <Show when={archiveError()}>
            <p class="font-mono text-xs text-signal-red">{archiveError()}</p>
          </Show>
        </Show>

        {/* Privacy disclosure */}
        <section class="border border-container-border bg-container-bg px-6 py-5">
          <div class="flex items-center gap-2 mb-3">
            <p class="font-mono text-[10px] tracking-wider text-text-tertiary uppercase">
              // PRIVACY
            </p>
            <HelpTip text={HELP['coach.provider'] ?? ''} />
          </div>
          <p class="font-display text-sm text-text-secondary leading-relaxed">
            Coach narratives are produced by an external LLM provider
            <Show when={latest()?.data?.model_used}>
              {' '}(model: <span class="font-mono">{latest()!.data!.model_used}</span>)
            </Show>
            . Only a compact weekly digest — aggregated baseline stats plus the specific trades
            flagged by the pattern detectors — is sent. Raw trade history outside the flagged set
            never leaves the server. You can disable the coach at any time with the toggle above.
          </p>
        </section>

        {/* Agent setup link */}
        <section class="border border-container-border bg-container-bg px-6 py-5">
          <div class="flex items-center gap-2 mb-3">
            <p class="font-mono text-[10px] tracking-wider text-text-tertiary uppercase">
              // AGENT_SETUP
            </p>
            <HelpTip text={HELP['coach.agent_setup'] ?? ''} />
          </div>
          <p class="font-display text-sm text-text-secondary leading-relaxed mb-4">
            AI agents (Claude, Hermes, OpenClaw, pi) can trade autonomously on Testudo using the
            same infrastructure. The agent reads its journal, submits signals, monitors fills via
            WebSocket, and writes theses and postmortems back — a complete closed loop.
          </p>
          <a
            href="https://github.com/m0xu/testudo/blob/master/AGENT_TRADING.md"
            target="_blank"
            rel="noopener noreferrer"
            class="inline-flex items-center gap-2 px-4 py-2 border border-text-primary text-text-primary font-mono text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M9 19c-5 1.5-5-2.5-7-3m14 6v-3.87a3.37 3.37 0 0 0-.94-2.61c3.14-.35 6.44-1.54 6.44-7A5.44 5.44 0 0 0 20 4.77 5.07 5.07 0 0 0 19.91 1S18.73.65 16 2.48a13.38 13.38 0 0 0-7 0C6.27.65 5.09 1 5.09 1A5.07 5.07 0 0 0 5 4.77a5.44 5.44 0 0 0-1.5 3.78c0 5.42 3.3 6.61 6.44 7A3.37 3.37 0 0 0 9 18.13V22" />
            </svg>
            VIEW ON GITHUB
          </a>
        </section>
      </div>
    </div>
  )
}
