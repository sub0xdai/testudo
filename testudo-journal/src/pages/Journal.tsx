import { JournalTimeline } from '../components/journal/JournalTimeline'

export function Journal() {
  return (
    <div class="flex flex-col h-full overflow-y-auto">
      <div class="px-8 py-5 shrink-0 border-b border-container-border/50 bg-container-bg">
        <h1 class="text-2xl md:text-3xl font-display font-bold tracking-tight">JOURNAL</h1>
      </div>
      <div class="flex-1 min-h-0 bg-container-bg">
        <JournalTimeline />
      </div>
    </div>
  )
}
