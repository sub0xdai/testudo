import type { JournalTag } from '../../api/client'

const DEFAULT_COLORS = ['#00FF41', '#FF003C', '#3B82F6', '#F59E0B', '#8B5CF6', '#EC4899', '#06B6D4', '#10B981']

function tagColor(tag: JournalTag, index: number): string {
  return tag.color || DEFAULT_COLORS[index % DEFAULT_COLORS.length]
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
