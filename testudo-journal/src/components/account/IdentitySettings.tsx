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

  async function toggleVisibility(field: 'show_score' | 'show_sparkline') {
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
    <section class="border border-container-border bg-container-bg">
      <div class="px-6 py-4 border-b border-container-border/50">
        <div class="font-mono text-[10px] tracking-widest text-text-tertiary">
          // IDENTITY
        </div>
        <h2 class="font-mono text-sm font-bold text-text-primary tracking-wider mt-1">
          PUBLIC PROFILE
        </h2>
      </div>

      <Show when={!identity.loading} fallback={
        <div class="flex items-center justify-center py-10">
          <div class="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
        </div>
      }>
        <Show when={identity()}>
          {(prefs) => (
            <div class="px-6 py-6 space-y-8">

              {/* ── No handle: claim form ── */}
              <Show when={!prefs().handle}>
                <div>
                  <p class="font-mono text-xs text-text-secondary mb-4 leading-relaxed">
                    Claim a globally-unique handle to create a shareable public profile at{' '}
                    <span class="text-text-primary">/desk/d/&lt;handle&gt;</span>.{' '}
                    Default is fully private — your handle reservation reveals nothing until you opt in.
                  </p>

                  <form onSubmit={handleClaim} class="space-y-4">
                    <div>
                      <label for="handle-input" class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                        HANDLE
                      </label>
                      <input
                        id="handle-input"
                        type="text"
                        value={claimInput()}
                        onInput={(e) => { setClaimInput(e.currentTarget.value); setClaimError('') }}
                        placeholder="e.g. 0xwhale"
                        maxlength={24}
                        autocomplete="off"
                        spellcheck={false}
                        class="w-full px-4 py-3 bg-main-bg/50 border border-container-border font-mono text-sm text-text-primary focus:border-text-secondary focus:outline-none placeholder:text-text-tertiary"
                      />
                      <Show when={handleInputHint()}>
                        <p class="font-mono text-[10px] text-signal-amber mt-1">{handleInputHint()}</p>
                      </Show>
                    </div>

                    <div>
                      <label for="claim-bio" class="block font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                        BIO <span class="text-text-tertiary/60">(optional · max 140)</span>
                      </label>
                      <textarea
                        id="claim-bio"
                        value={claimBio()}
                        onInput={(e) => setClaimBio(e.currentTarget.value)}
                        placeholder="One line about your edge"
                        maxlength={140}
                        rows={2}
                        class="w-full px-4 py-3 bg-main-bg/50 border border-container-border font-mono text-sm text-text-primary focus:border-text-secondary focus:outline-none placeholder:text-text-tertiary resize-none"
                      />
                      <p class="font-mono text-[10px] text-text-tertiary text-right mt-1">
                        {claimBio().length}/140
                      </p>
                    </div>

                    <Show when={claimError()}>
                      <p class="font-mono text-xs text-signal-red">{claimError()}</p>
                    </Show>

                    <button
                      type="submit"
                      disabled={claiming() || !claimInput().trim()}
                      class="px-6 py-2 border border-text-primary font-mono text-xs font-bold tracking-wider text-text-primary hover:bg-text-primary hover:text-main-bg transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                    >
                      {claiming() ? 'CLAIMING…' : 'CLAIM HANDLE'}
                    </button>
                  </form>
                </div>
              </Show>

              {/* ── Has handle: management panel ── */}
              <Show when={prefs().handle}>
                {/* Handle row */}
                <div class="flex items-center justify-between gap-4">
                  <div>
                    <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-1">HANDLE</div>
                    <div class="font-mono text-lg font-bold text-text-primary">{prefs().handle}</div>
                    <div class="font-mono text-[10px] text-text-tertiary mt-0.5">
                      /desk/d/{prefs().handle}
                    </div>
                  </div>

                  <div class="flex flex-col items-end gap-1">
                    <Show when={releaseError()}>
                      <p class="font-mono text-[10px] text-signal-red">{releaseError()}</p>
                    </Show>
                    <Show
                      when={canChangeHandle(prefs())}
                      fallback={
                        <span class="font-mono text-[10px] text-text-tertiary text-right max-w-48">
                          can change after {formatRetryAt(prefs().can_change_handle_at!)}
                        </span>
                      }
                    >
                      <Show
                        when={confirmRelease()}
                        fallback={
                          <button
                            onClick={() => setConfirmRelease(true)}
                            class="font-mono text-[10px] tracking-wider text-text-tertiary hover:text-signal-red transition-colors"
                          >
                            RELEASE
                          </button>
                        }
                      >
                        <div class="flex items-center gap-3">
                          <span class="font-mono text-[10px] text-signal-red">confirm release?</span>
                          <button
                            onClick={handleRelease}
                            disabled={releasing()}
                            class="font-mono text-[10px] tracking-wider text-signal-red hover:underline disabled:opacity-50"
                          >
                            YES
                          </button>
                          <button
                            onClick={() => setConfirmRelease(false)}
                            class="font-mono text-[10px] tracking-wider text-text-tertiary hover:text-text-primary"
                          >
                            NO
                          </button>
                        </div>
                      </Show>
                    </Show>
                  </div>
                </div>

                {/* Bio */}
                <div>
                  <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-2">BIO</div>
                  <Show
                    when={bioEdit() !== null}
                    fallback={
                      <div class="flex items-start justify-between gap-4">
                        <p class="font-mono text-sm text-text-secondary">
                          {prefs().bio ?? <span class="text-text-tertiary italic">no bio</span>}
                        </p>
                        <button
                          onClick={startBioEdit}
                          class="font-mono text-[10px] tracking-wider text-text-tertiary hover:text-text-primary transition-colors shrink-0"
                        >
                          EDIT
                        </button>
                      </div>
                    }
                  >
                    <div class="space-y-2">
                      <textarea
                        value={bioEdit() ?? ''}
                        onInput={(e) => setBioEdit(e.currentTarget.value)}
                        maxlength={140}
                        rows={2}
                        class="w-full px-4 py-3 bg-main-bg/50 border border-container-border font-mono text-sm text-text-primary focus:border-text-secondary focus:outline-none resize-none"
                      />
                      <div class="flex items-center justify-between gap-4">
                        <p class="font-mono text-[10px] text-text-tertiary">{(bioEdit() ?? '').length}/140</p>
                        <div class="flex items-center gap-3">
                          <Show when={bioError()}>
                            <span class="font-mono text-[10px] text-signal-red">{bioError()}</span>
                          </Show>
                          <button
                            onClick={() => setBioEdit(null)}
                            class="font-mono text-[10px] tracking-wider text-text-tertiary hover:text-text-primary"
                          >
                            CANCEL
                          </button>
                          <button
                            onClick={saveBio}
                            disabled={bioSaving()}
                            class="font-mono text-[10px] tracking-wider text-text-primary hover:underline disabled:opacity-50"
                          >
                            {bioSaving() ? 'SAVING…' : 'SAVE'}
                          </button>
                        </div>
                      </div>
                    </div>
                  </Show>
                </div>

                {/* Visibility toggles */}
                <div>
                  <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-4">VISIBILITY</div>
                  <div class="space-y-3">
                    <ToggleRow
                      label="Show Dignitas score"
                      hint="Displays your current score on the public profile"
                      enabled={prefs().show_score}
                      onToggle={() => toggleVisibility('show_score')}
                    />
                    <ToggleRow
                      label="Show 90-day sparkline"
                      hint="Shows score history chart on the public profile"
                      enabled={prefs().show_sparkline}
                      onToggle={() => toggleVisibility('show_sparkline')}
                    />
                    <ToggleRow
                      label="Allow search engine indexing"
                      hint="Opts profile in to JS-crawler indexing (noindex default)"
                      enabled={prefs().allow_indexing}
                      onToggle={toggleIndexing}
                    />
                  </div>
                  <Show when={visError()}>
                    <p class="font-mono text-[10px] text-signal-red mt-2">{visError()}</p>
                  </Show>
                </div>
              </Show>

            </div>
          )}
        </Show>
      </Show>
    </section>
  )
}

interface ToggleRowProps {
  label: string
  hint: string
  enabled: boolean
  onToggle: () => void
}

function ToggleRow(props: ToggleRowProps) {
  return (
    <div class="flex items-center justify-between gap-4">
      <div>
        <div class="font-mono text-xs text-text-primary">{props.label}</div>
        <div class="font-mono text-[10px] text-text-tertiary mt-0.5">{props.hint}</div>
      </div>
      <button
        onClick={props.onToggle}
        role="switch"
        aria-checked={props.enabled}
        class="font-mono text-[10px] tracking-wider px-3 py-1.5 border transition-colors shrink-0"
        classList={{
          'border-text-primary text-text-primary': props.enabled,
          'border-container-border text-text-tertiary hover:text-text-primary': !props.enabled,
        }}
      >
        {props.enabled ? 'ON' : 'OFF'}
      </button>
    </div>
  )
}
