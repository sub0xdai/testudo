import type { JSX } from 'solid-js'
import { A } from '@solidjs/router'
import { routePrefetchers } from '../lib/route-prefetch'

interface NavLinkProps {
  href: string
  end?: boolean
  class?: string
  activeClass?: string
  inactiveClass?: string
  onClick?: () => void
  children: JSX.Element
}

function isSlow(): boolean {
  const conn = (navigator as any).connection
  return conn?.saveData === true || /^(2g|slow-2g)$/i.test(conn?.effectiveType ?? '')
}

export function NavLink(props: NavLinkProps) {
  function handlePrefetch() {
    if (isSlow()) return
    routePrefetchers[props.href]?.()
  }

  return (
    <A
      href={props.href}
      end={props.end}
      class={props.class}
      activeClass={props.activeClass}
      inactiveClass={props.inactiveClass}
      onClick={props.onClick}
      onMouseEnter={handlePrefetch}
      onTouchStart={handlePrefetch}
    >
      {props.children}
    </A>
  )
}
