import type { JournalTag } from '../../api/client'
import { getTagPalette } from '../../lib/tokens'

function tagColor(tag: JournalTag, index: number): string {
  if (tag.color) return tag.color
  const palette = getTagPalette()
  return palette[index % palette.length]
}

export function TagBadge(props: { tag: JournalTag; index?: number; onRemove?: () => void }) {
  const color = () => tagColor(props.tag, props.index ?? 0)

  return (
    <span
      class="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-mono border rounded"
      style={{ 'border-color': color(), color: color() }}
    >
      <span class="w-1.5 h-1.5 rounded-full" style={{ background: color() }} />
      {props.tag.name}
      {props.onRemove && (
        <button
          class="ml-0.5 opacity-50 hover:opacity-100 transition-opacity"
          onClick={(e) => { e.stopPropagation(); props.onRemove!() }}
        >
          &times;
        </button>
      )}
    </span>
  )
}
