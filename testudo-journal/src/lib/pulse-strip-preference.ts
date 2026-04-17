import { createSignal } from 'solid-js'

const STORAGE_KEY = 'testudo-pulse-strip'

function readStored(): boolean {
  try {
    return localStorage.getItem(STORAGE_KEY) !== 'off'
  } catch {
    return true
  }
}

const [enabled, setEnabled] = createSignal(readStored())

export const pulseStripEnabled = enabled

export function setPulseStripEnabled(next: boolean) {
  setEnabled(next)
  try {
    localStorage.setItem(STORAGE_KEY, next ? 'on' : 'off')
  } catch {
    // Preference write is best-effort — localStorage may be blocked in private mode.
  }
}
