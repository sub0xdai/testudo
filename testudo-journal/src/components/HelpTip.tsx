import { createSignal, Show } from 'solid-js'

interface HelpTipProps {
  text: string
  position?: 'above' | 'below' | 'left' | 'right'
}

export function HelpTip(props: HelpTipProps) {
  const [show, setShow] = createSignal(false)
  const pos = () => props.position ?? 'above'

  if (!props.text) return null

  return (
    <span
      class="help-tip"
      onMouseEnter={() => setShow(true)}
      onMouseLeave={() => setShow(false)}
      onFocus={() => setShow(true)}
      onBlur={() => setShow(false)}
      onClick={(e) => e.stopPropagation()}
      tabIndex={0}
      role="button"
      aria-label="Help"
    >
      <span class="help-tip-trigger" aria-hidden="true">?</span>
      <Show when={show()}>
        <span class={`help-tip-popup help-tip-${pos()}`} role="tooltip">
          {props.text}
        </span>
      </Show>
    </span>
  )
}
