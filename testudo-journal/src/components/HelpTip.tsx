/** @anchor ui:journal:HelpTip
 * @tags ui */

import { createSignal, Show } from 'solid-js'
import { Portal } from 'solid-js/web'

interface HelpTipProps {
  text: string
  position?: 'above' | 'below' | 'left' | 'right'
}

export function HelpTip(props: HelpTipProps) {
  const [show, setShow] = createSignal(false)
  const [rect, setRect] = createSignal<DOMRect | null>(null)
  let triggerRef: HTMLSpanElement | undefined
  const pos = () => props.position ?? 'above'

  if (!props.text) return null

  function updateRect() {
    if (triggerRef) setRect(triggerRef.getBoundingClientRect())
  }

  function popupStyle(): Record<string, string> {
    const r = rect()
    if (!r) return {}
    const p = pos()
    if (p === 'below') {
      return {
        position: 'fixed',
        top: `${r.bottom + 6}px`,
        left: `${r.left}px`,
      }
    }
    if (p === 'above') {
      return {
        position: 'fixed',
        bottom: `${window.innerHeight - r.top + 6}px`,
        left: `${r.left}px`,
      }
    }
    if (p === 'right') {
      return {
        position: 'fixed',
        top: `${r.top + r.height / 2}px`,
        left: `${r.right + 6}px`,
        transform: 'translateY(-50%)',
      }
    }
    // left
    return {
      position: 'fixed',
      top: `${r.top + r.height / 2}px`,
      right: `${window.innerWidth - r.left + 6}px`,
      transform: 'translateY(-50%)',
    }
  }

  return (
    <span
      class="help-tip"
      onMouseEnter={() => { updateRect(); setShow(true) }}
      onMouseLeave={() => setShow(false)}
      onFocus={() => { updateRect(); setShow(true) }}
      onBlur={() => setShow(false)}
      onClick={(e) => e.stopPropagation()}
      tabIndex={0}
      role="button"
      aria-label="Help"
    >
      <span class="help-tip-trigger" ref={triggerRef} aria-hidden="true">?</span>
      <Show when={show() && rect()}>
        <Portal>
          <span class="help-tip-popup" style={popupStyle()} role="tooltip">
            {props.text}
          </span>
        </Portal>
      </Show>
    </span>
  )
}
