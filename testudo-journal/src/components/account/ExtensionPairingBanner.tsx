import { createSignal, onCleanup, Show } from 'solid-js'
import { pairExtension } from '../../api/client'

const CODE_TTL_SECONDS = 300

export function ExtensionPairingBanner() {
  const [code, setCode] = createSignal<string | null>(null)
  const [countdown, setCountdown] = createSignal(0)
  const [generating, setGenerating] = createSignal(false)
  const [error, setError] = createSignal('')
  const [copied, setCopied] = createSignal(false)

  let timer: ReturnType<typeof setInterval> | null = null
  let copyTimeout: ReturnType<typeof setTimeout> | null = null

  function clearTimer() {
    if (timer) {
      clearInterval(timer)
      timer = null
    }
  }

  onCleanup(() => {
    clearTimer()
    if (copyTimeout) clearTimeout(copyTimeout)
  })

  async function generateCode() {
    setGenerating(true)
    setError('')
    try {
      const { code: newCode } = await pairExtension()
      setCode(newCode)
      setCountdown(CODE_TTL_SECONDS)

      clearTimer()
      timer = setInterval(() => {
        setCountdown((prev) => {
          if (prev <= 1) {
            clearTimer()
            setCode(null)
            return 0
          }
          return prev - 1
        })
      }, 1000)
    } catch {
      setError('Failed to generate pairing code')
    } finally {
      setGenerating(false)
    }
  }

  function copyCode() {
    const current = code()
    if (!current) return
    navigator.clipboard.writeText(current)
    setCopied(true)
    if (copyTimeout) clearTimeout(copyTimeout)
    copyTimeout = setTimeout(() => setCopied(false), 1500)
  }

  const minutes = () => Math.floor(countdown() / 60)
  const seconds = () => String(countdown() % 60).padStart(2, '0')

  return (
    <div class="mt-8 border-t border-container-border pt-6">
      <Show
        when={code()}
        fallback={
          <div class="flex items-center justify-between gap-4 flex-wrap">
            <div>
              <h3 class="font-display text-sm font-bold text-text-primary">
                EXTENSION PAIRING
              </h3>
              <p class="font-mono text-xs text-text-tertiary mt-0.5">
                Generate a code to link the Testudo browser extension.
              </p>
            </div>
            <button
              onClick={generateCode}
              disabled={generating()}
              class="px-4 py-2 font-mono text-xs font-bold text-text-primary border border-container-border hover:bg-text-primary hover:text-main-bg transition-colors disabled:opacity-50 whitespace-nowrap"
            >
              {generating() ? 'GENERATING...' : 'PAIR EXTENSION'}
            </button>
          </div>
        }
      >
        <div class="flex items-center justify-between gap-6 flex-wrap">
          <div class="flex items-center gap-4">
            <h3 class="font-display text-sm font-bold text-text-primary whitespace-nowrap">
              EXTENSION PAIRING
            </h3>
            <button
              onClick={copyCode}
              title="Click to copy"
              class="font-mono text-2xl font-bold text-text-primary tracking-[0.3em] hover:opacity-60 transition-opacity cursor-pointer"
            >
              {copied() ? '\u2713 COPIED' : code()}
            </button>
            <span class="font-mono text-xs text-text-tertiary">
              {minutes()}:{seconds()}
            </span>
          </div>
          <button
            onClick={generateCode}
            disabled={generating()}
            class="px-4 py-2 font-mono text-xs text-text-primary border border-text-tertiary hover:border-text-primary transition-colors disabled:opacity-50 whitespace-nowrap"
          >
            {generating() ? 'GENERATING...' : 'NEW CODE'}
          </button>
        </div>
      </Show>

      <Show when={error()}>
        <div class="mt-3 px-4 py-2 border border-signal-red bg-signal-red/10 font-mono text-xs text-signal-red">
          {error()}
        </div>
      </Show>
    </div>
  )
}
