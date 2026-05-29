/** @anchor ui:journal:NarrativeBlock
 * @tags ui */

import { For, createMemo } from 'solid-js'
import { marked } from 'marked'
import DOMPurify from 'dompurify'
import type { CoachNarrativeSection, CoachTradeEvidence, CoachPatternKind } from '../../api/client'
import { HelpTip } from '../HelpTip'
import { HELP } from '../../lib/help-content'

marked.setOptions({ breaks: true, gfm: true })

const PATTERN_LABELS: Record<CoachPatternKind, string> = {
  sizing_drift: 'SIZING DRIFT',
  frequency_spike: 'FREQUENCY SPIKE',
  session_anomaly: 'SESSION ANOMALY',
  setup_fatigue: 'SETUP FATIGUE',
  correlation_stack: 'CORRELATION STACK',
  streak_risk: 'STREAK RISK',
}

const CITATION_REGEX = /\[T-([0-9a-f]{8})\]/g

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    c === '&' ? '&amp;' : c === '<' ? '&lt;' : c === '>' ? '&gt;' : c === '"' ? '&quot;' : '&#39;',
  )
}

function replaceCitations(body: string, flagged: CoachTradeEvidence[]): string {
  return body.replace(CITATION_REGEX, (match, short) => {
    const trade = flagged.find((t) => t.short_id === short)
    if (!trade) return match
    const href = `/desk/trades?trade=${encodeURIComponent(trade.id)}`
    const title = `${escapeHtml(trade.symbol)} · ${escapeHtml(trade.side)}`
    return `<a href="${href}" class="coach-citation" title="${title}">T-${short}</a>`
  })
}

interface NarrativeBlockProps {
  sections: CoachNarrativeSection[]
  flagged: CoachTradeEvidence[]
}

export function NarrativeBlock(props: NarrativeBlockProps) {
  return (
    <div class="flex flex-col gap-6">
      <For each={props.sections}>
        {(section) => <NarrativeSectionView section={section} flagged={props.flagged} />}
      </For>
    </div>
  )
}

function NarrativeSectionView(props: { section: CoachNarrativeSection; flagged: CoachTradeEvidence[] }) {
  const html = createMemo(() => {
    const linked = replaceCitations(props.section.body, props.flagged)
    const rendered = marked.parse(linked, { async: false }) as string
    return DOMPurify.sanitize(rendered, {
      ADD_ATTR: ['target', 'rel', 'title', 'class'],
    })
  })

  const label = () => PATTERN_LABELS[props.section.pattern] ?? props.section.pattern
  const helpKey = () => `coach.patterns.${props.section.pattern}`

  return (
    <section class="border border-container-border bg-container-bg">
      <header class="flex items-center gap-2 px-4 py-3 border-b border-container-border/60">
        <span class="font-display text-[10px] font-bold tracking-section text-text-secondary uppercase">
          {label()}
        </span>
        <HelpTip text={HELP[helpKey()] ?? ''} />
      </header>
      <div
        class="markdown-preview font-display text-sm text-text-secondary leading-relaxed px-4 py-4"
        innerHTML={html()}
      />
    </section>
  )
}
