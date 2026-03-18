import { JournalTimeline } from '../components/journal/JournalTimeline'
import { GhostAnnotation } from '../components/GhostAnnotation'

export function Journal() {
  return (
    <div>
      <div class="mb-4">
        <GhostAnnotation text="JOURNAL_ENTRIES" />
        <h1 class="text-2xl md:text-3xl font-display font-bold tracking-tight">JOURNAL</h1>
      </div>
      <JournalTimeline />
    </div>
  )
}
