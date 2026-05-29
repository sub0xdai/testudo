/** @anchor infra:journal-lib:createFocusTrap
 * @tags infra */

import { onMount, onCleanup } from 'solid-js'

const FOCUSABLE = [
  'a[href]', 'button:not([disabled])', 'input:not([disabled])',
  'select:not([disabled])', 'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ')

export function createFocusTrap(containerRef: () => HTMLElement | undefined) {
  let previouslyFocused: HTMLElement | null = null

  function trapFocus(e: KeyboardEvent) {
    if (e.key !== 'Tab') return
    const container = containerRef()
    if (!container) return

    const focusable = Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE))
    if (focusable.length === 0) return

    const first = focusable[0]
    const last = focusable[focusable.length - 1]

    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault()
      first.focus()
    }
  }

  onMount(() => {
    previouslyFocused = document.activeElement as HTMLElement | null
    document.addEventListener('keydown', trapFocus)
    // Focus first focusable element on open
    const container = containerRef()
    const first = container?.querySelector<HTMLElement>(FOCUSABLE)
    first?.focus()
  })

  onCleanup(() => {
    document.removeEventListener('keydown', trapFocus)
    // Restore focus to trigger element
    previouslyFocused?.focus()
  })
}
