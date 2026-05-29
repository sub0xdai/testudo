/** @anchor ui:journal-page:Journal
 * @tags ui */

import { JournalTimeline } from '../components/journal/JournalTimeline'
import { HelpTip } from '../components/HelpTip'
import { HELP } from '../lib/help-content'

export function Journal() {
  return (
    <div class="flex flex-col h-full overflow-y-auto">
      <div class="px-8 py-5 shrink-0 border-b border-container-border/50 bg-container-bg">
        <h1 class="font-display text-lg font-bold tracking-wider">
          JOURNAL
          <HelpTip text={HELP['page.entries']} position="below" />
        </h1>
      </div>
      <div class="flex-1 min-h-0 bg-container-bg">
        <JournalTimeline />
      </div>
    </div>
  )
}
