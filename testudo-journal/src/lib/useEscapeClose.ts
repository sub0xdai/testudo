/** @anchor infra:journal-lib:useEscapeClose
 * @tags infra */

import { onMount, onCleanup } from 'solid-js'

export function useEscapeClose(onClose: () => void) {
  const handler = (e: KeyboardEvent) => {
    if (e.key === 'Escape') onClose()
  }
  onMount(() => window.addEventListener('keydown', handler))
  onCleanup(() => window.removeEventListener('keydown', handler))
}
