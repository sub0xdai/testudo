import type { JournalEntry, JournalTag } from '../api/client'

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
}

export function exportEntry(entry: JournalEntry, tags?: JournalTag[]) {
  const lines = [
    '---',
    `title: "${entry.title}"`,
    `date: ${entry.created_at}`,
    `type: ${entry.entry_type}`,
  ]
  if (tags?.length) {
    lines.push(`tags: [${tags.map((t) => `"${t.name}"`).join(', ')}]`)
  }
  lines.push('---', '', entry.body)

  const content = lines.join('\n')
  const blob = new Blob([content], { type: 'text/markdown' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `${entry.created_at.slice(0, 10)}-${slugify(entry.title)}.md`
  a.click()
  URL.revokeObjectURL(url)
}
