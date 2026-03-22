import type { JournalEntry, JournalTag } from '../api/client'

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve) => {
    const reader = new FileReader()
    reader.onloadend = () => resolve(reader.result as string)
    reader.readAsDataURL(blob)
  })
}

async function inlineImages(markdown: string): Promise<string> {
  const imageRegex = /!\[([^\]]*)\]\(([^)]+)\)/g
  const matches = [...markdown.matchAll(imageRegex)]

  let result = markdown
  for (const match of matches) {
    const [full, alt, url] = match
    if (url.startsWith('data:')) continue
    try {
      const res = await fetch(url)
      const blob = await res.blob()
      const base64 = await blobToBase64(blob)
      result = result.replace(full, `![${alt}](${base64})`)
    } catch {
      // Leave original URL if fetch fails — graceful degradation
    }
  }
  return result
}

function buildFrontmatter(entry: JournalEntry, tags?: JournalTag[]): string {
  const lines = [
    '---',
    `title: "${entry.title}"`,
    `date: ${entry.created_at}`,
    `type: ${entry.entry_type}`,
  ]
  if (tags?.length) {
    lines.push(`tags: [${tags.map((t) => `"${t.name}"`).join(', ')}]`)
  }
  lines.push('---')
  return lines.join('\n')
}

function downloadBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

export async function exportEntry(entry: JournalEntry, tags?: JournalTag[]) {
  const frontmatter = buildFrontmatter(entry, tags)
  const body = await inlineImages(entry.body)
  const content = frontmatter + '\n' + body
  const blob = new Blob([content], { type: 'text/markdown' })
  downloadBlob(blob, `${entry.created_at.slice(0, 10)}-${slugify(entry.title)}.md`)
}

export async function exportEntries(
  entries: JournalEntry[],
  tagMap: Record<string, JournalTag[]>,
  onProgress?: (current: number, total: number) => void,
): Promise<void> {
  const sections: string[] = []
  for (let i = 0; i < entries.length; i++) {
    const entry = entries[i]
    const tags = tagMap[entry.id] ?? []
    const frontmatter = buildFrontmatter(entry, tags)
    const body = await inlineImages(entry.body)
    sections.push(frontmatter + '\n' + body)
    onProgress?.(i + 1, entries.length)
  }
  const content = sections.join('\n\n---\n\n')
  const blob = new Blob([content], { type: 'text/markdown' })
  downloadBlob(blob, `testudo-journal-export-${new Date().toISOString().slice(0, 10)}.md`)
}
