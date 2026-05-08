import { createResource, createSignal, Show, batch } from 'solid-js'
import {
  fetchIdentity,
  claimHandle,
  releaseHandle,
  patchVisibility,
  patchIndexing,
  updateBio,
  type IdentityPreferences,
} from '../../api/client'
import { RESERVED_HANDLES } from '../../config/dignitas-reserved-handles'

const HANDLE_RE = /^[a-z0-9][a-z0-9_-]{1,22}[a-z0-9]$/

function formatRetryAt(iso: string): string {
  const d = new Date(iso)
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })
}

function localHandleError(handle: string): string | null {
  const h = handle.trim().toLowerCase()
  if (h.length < 3) return 'too short (min 3 chars)'
  if (h.length > 24) return 'too long (max 24 chars)'
  if (!HANDLE_RE.test(h)) return 'must start/end alphanumeric, may contain _ or -'
  if (RESERVED_HANDLES.has(h)) return 'reserved word'
  return null
}

export function IdentitySettings() {
  const [identity, { mutate, refetch }] = createResource(fetchIdentity)

  // Claim form
  const [claimInput, setClaimInput] = createSignal('')
  const [claimBio, setClaimBio] = createSignal('')
  const [claimError, setClaimError] = createSignal('')
  const [claiming, setClaiming] = createSignal(false)

  // Release
  const [releasing, setReleasing] = createSignal(false)
  const [releaseError, setReleaseError] = createSignal('')
  const [confirmRelease, setConfirmRelease] = createSignal(false)

  // Bio edit
  const [bioEdit, setBioEdit] = createSignal<string | null>(null)
  const [bioSaving, setBioSaving] = createSignal(false)
  const [bioError, setBioError] = createSignal('')

  // Visibility save errors
  const [visError, setVisError] = createSignal('')

  const handleInputHint = () => {
    const v = claimInput().trim()
    if (!v) return ''
    return localHandleError(v) ?? ''
  }

  async function handleClaim(e: Event) {
    e.preventDefault()
    const h = claimInput().trim().toLowerCase()
    const localErr = localHandleError(h)
    if (localErr) { setClaimError(localErr); return }
    const bio = claimBio().trim() || undefined
    setClaiming(true)
    setClaimError('')
    try {
      const prefs = await claimHandle(h, bio)
      batch(() => {
        mutate(prefs)
        setClaimInput('')
        setClaimBio('')
      })
    } catch (err: unknown) {
      const e = err as { code?: string; status?: number; message?: string; data?: { retry_at?: string } }
      if (e.code === 'rate_limited' || e.status === 429) {
        const retryAt = e.data?.retry_at
        setClaimError(retryAt
          ? `handle change rate-limited until ${formatRetryAt(retryAt)}`
          : 'too many handle changes — wait 30 days')
      } else if (e.status === 409) {
        setClaimError('handle already taken')
      } else if (e.status === 400) {
        setClaimError(e.message ?? 'invalid handle')
      } else {
        setClaimError(e.message ?? 'failed to claim handle')
      }
    } finally {
      setClaiming(false)
    }
  }

  async function handleRelease() {
    if (!confirmRelease()) { setConfirmRelease(true); return }
    setReleasing(true)
    setReleaseError('')
    setConfirmRelease(false)
    try {
      await releaseHandle()
      refetch()
    } catch (err: unknown) {
      const e = err as { code?: string; status?: number; message?: string; data?: { retry_at?: string } }
      if (e.code === 'rate_limited' || e.status === 429) {
        const retryAt = e.data?.retry_at
        setReleaseError(retryAt
          ? `rate-limited until ${formatRetryAt(retryAt)}`
          : 'too many handle changes — wait 30 days')
      } else {
        setReleaseError(e.message ?? 'failed to release handle')
      }
    } finally {
      setReleasing(false)
    }
  }

  function startBioEdit() {
    setBioEdit(identity()?.bio ?? '')
    setBioError('')
  }

  async function saveBio() {
    const val = bioEdit()
    if (val === null) return
    const trimmed = val.trim() || null
    setBioSaving(true)
    setBioError('')
    try {
      await updateBio(trimmed)
      mutate(prev => prev ? { ...prev, bio: trimmed } : prev)
      setBioEdit(null)
    } catch (err: unknown) {
      setBioError((err as Error).message ?? 'failed to save bio')
    } finally {
      setBioSaving(false)
    }
  }

  async function toggleVisibility(field: 'show_score' | 'show_sparkline' | 'show_streak') {
    const cur = identity()
    if (!cur) return
    const next = !cur[field]
    mutate({ ...cur, [field]: next })
    setVisError('')
    try {
      await patchVisibility({ [field]: next })
    } catch {
      mutate(cur)
      setVisError(`failed to update ${field}`)
    }
  }

  async function toggleIndexing() {
    const cur = identity()
    if (!cur) return
    const next = !cur.allow_indexing
    mutate({ ...cur, allow_indexing: next })
    setVisError('')
    try {
      await patchIndexing(next)
    } catch {
      mutate(cur)
      setVisError('failed to update indexing preference')
    }
  }

  const canChangeHandle = (prefs: IdentityPreferences) =>
    prefs.can_change_handle_at === null || new Date(prefs.can_change_handle_at) <= new Date()

  return (
    <section class="opacity-80 hover:opacity-100 transition-opacity">
      <div class="mb-8 flex items-center gap-4">
        <h2 class="font-mono text-[10px] font-bold text-text-tertiary tracking-[0.2em] uppercase whitespace-nowrap">
          // IDENTITY_PROFILE
        </h2>
        <div class="h-[1px] w-full bg-container-border/30" />
      </div>

      <Show when={!identity.loading} fallback={
        <div class="py-4">
          <div class="w-3 h-3 border border-text-tertiary border-t-text-primary rounded-full animate-spin" />
        </div>
      }>
        <Show when={identity()}>
          {(prefs) => (
            <div class="grid grid-cols-1 lg:grid-cols-12 gap-12">
              
              {/* ── Identity Context ── */}
              <div class="lg:col-span-7 space-y-8">
                <Show when={!prefs().handle}>
                  <div class="max-w-md">
                    <p class="font-mono text-[10px] text-text-tertiary mb-4 leading-relaxed uppercase tracking-wider">
                      RESERVE A UNIQUE HANDLE TO ENABLE PUBLIC PERFORMANCE TRACKING.
                    </p>
                    <form onSubmit={handleClaim} class="flex gap-2">
                      <input
                        id="handle-input"
                        type="text"
                        value={claimInput()}
                        onInput={(e) => { setClaimInput(e.currentTarget.value); setClaimError('') }}
                        placeholder="HANDLE"
                        maxlength={24}
                        class="flex-1 px-3 py-1.5 bg-transparent border border-container-border/50 font-mono text-[10px] text-text-primary focus:border-text-secondary focus:outline-none placeholder:text-text-tertiary/50"
                      />
                      <button
                        type="submit"
                        disabled={claiming() || !claimInput().trim()}
                        class="px-4 py-1.5 border border-text-tertiary/50 font-mono text-[10px] text-text-tertiary hover:text-text-primary hover:border-text-primary transition-colors"
                      >
                        CLAIM
                      </button>
                    </form>
                  </div>
                </Show>

                <Show when={prefs().handle}>
                  <div class="flex flex-col md:flex-row md:items-start gap-8 md:gap-16">
                    <div class="shrink-0">
                      <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-2 uppercase">HANDLE</div>
                      <div class="flex items-center gap-3">
                        <span class="font-mono text-sm font-bold text-text-secondary">@{prefs().handle}</span>
                        <Show when={canChangeHandle(prefs())}>
                          <button onClick={handleRelease} class="font-mono text-[10px] text-text-tertiary/50 hover:text-signal-red transition-colors">
                            [×]
                          </button>
                        </Show>
                      </div>
                    </div>

                    <div class="flex-1">
                      <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-2 uppercase">BIO</div>
                      <p class="font-mono text-xs text-text-tertiary leading-relaxed max-w-xl">
                        {prefs().bio ?? <span class="italic opacity-30">No biometric data recorded.</span>}
                      </p>
                    </div>
                  </div>
                </Show>
              </div>

              {/* ── Controls ── */}
              <div class="lg:col-span-5">
                <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-4 uppercase">VISIBILITY_PROTOCOL</div>
                <div class="grid grid-cols-1 sm:grid-cols-2 gap-x-8 gap-y-2">
                  <ToggleRow
                    label="DIGNITAS_SCORE"
                    enabled={prefs().show_score}
                    onToggle={() => toggleVisibility('show_score')}
                  />
                  <ToggleRow
                    label="90D_SPARKLINE"
                    enabled={prefs().show_sparkline}
                    onToggle={() => toggleVisibility('show_sparkline')}
                  />
                  <ToggleRow
                    label="DISCIPLINE_STREAK"
                    enabled={prefs().show_streak}
                    onToggle={() => toggleVisibility('show_streak')}
                  />
                  <ToggleRow
                    label="INDEXING"
                    enabled={prefs().allow_indexing}
                    onToggle={toggleIndexing}
                  />
                </div>
              </div>

            </div>
          )}
        </Show>
      </Show>
    </section>
  )
}

function ToggleRow(props: { label: string, enabled: boolean, onToggle: () => void }) {
  return (
    <div class="flex items-center justify-between py-1 border-b border-container-border/10">
      <span class="font-mono text-[10px] text-text-tertiary uppercase">{props.label}</span>
      <button
        onClick={props.onToggle}
        class={`font-mono text-[10px] hover:text-text-primary transition-colors ${
          props.enabled ? 'text-signal-green' : 'text-text-tertiary/40'
        }`}
      >
        {props.enabled ? '[ON]' : '[OFF]'}
      </button>
    </div>
  )
}


