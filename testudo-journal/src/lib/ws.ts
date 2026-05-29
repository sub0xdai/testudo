/** @anchor infra:journal-lib:ws
 * @tags infra */

import { createSignal, type Accessor } from 'solid-js'

// VITE_WS_URL points at the ws-stream crate (default :4000 in local dev).
const WS_URL = (import.meta.env.VITE_WS_URL as string | undefined) || 'ws://localhost:4000'
const BASE_DELAY_MS = 1000
const MAX_DELAY_MS = 30_000

export interface RiskWsHandle {
  connect(userId: string): void
  disconnect(): void
  connected: Accessor<boolean>
}

export function createRiskWsClient(onRiskEvent: () => void): RiskWsHandle {
  const [connected, setConnected] = createSignal(false)

  let ws: WebSocket | null = null
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null
  let delay = BASE_DELAY_MS
  let activeUserId: string | null = null
  let closedByUser = true

  function teardownSocket() {
    if (reconnectTimer) {
      clearTimeout(reconnectTimer)
      reconnectTimer = null
    }
    if (ws) {
      ws.onopen = null
      ws.onmessage = null
      ws.onclose = null
      ws.onerror = null
      try { ws.close() } catch { /* ignore */ }
      ws = null
    }
  }

  function scheduleReconnect() {
    if (closedByUser || reconnectTimer) return
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null
      delay = Math.min(delay * 2, MAX_DELAY_MS)
      open()
    }, delay)
  }

  function open() {
    if (closedByUser || !activeUserId) return
    const uid = activeUserId

    try {
      ws = new WebSocket(WS_URL)
    } catch {
      setConnected(false)
      scheduleReconnect()
      return
    }

    ws.onopen = () => {
      delay = BASE_DELAY_MS
      setConnected(true)
      try {
        ws?.send(JSON.stringify({ method: 'SUBSCRIBE', params: [`order.${uid}`], id: 1 }))
      } catch { /* ignore send errors; onclose will follow */ }
    }

    ws.onmessage = (ev: MessageEvent) => {
      try {
        const msg = typeof ev.data === 'string' ? JSON.parse(ev.data) : ev.data
        if (
          msg &&
          typeof msg === 'object' &&
          typeof msg.stream === 'string' &&
          msg.stream.startsWith('order.')
        ) {
          onRiskEvent()
        }
      } catch { /* ignore malformed frames */ }
    }

    ws.onclose = () => {
      ws = null
      setConnected(false)
      scheduleReconnect()
    }

    ws.onerror = () => { /* onclose follows */ }
  }

  return {
    connect(userId: string) {
      closedByUser = false
      activeUserId = userId
      delay = BASE_DELAY_MS
      teardownSocket()
      open()
    },
    disconnect() {
      closedByUser = true
      activeUserId = null
      teardownSocket()
      setConnected(false)
    },
    connected,
  }
}
