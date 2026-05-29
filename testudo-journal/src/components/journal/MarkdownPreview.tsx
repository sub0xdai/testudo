/** @anchor ui:journal:MarkdownPreview
 * @tags ui */

import { createMemo } from 'solid-js'
import { marked } from 'marked'
import DOMPurify from 'dompurify'

marked.setOptions({
  breaks: true,
  gfm: true,
})

export function MarkdownPreview(props: { content: string }) {
  const html = createMemo(() => {
    if (!props.content.trim()) return '<p class="text-text-tertiary">No content</p>'
    const raw = marked.parse(props.content, { async: false }) as string
    return DOMPurify.sanitize(raw)
  })

  return (
    <div
      class="markdown-preview font-display text-sm text-text-secondary leading-relaxed"
      innerHTML={html()}
    />
  )
}
