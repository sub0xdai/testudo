import { createResource, createEffect, Show, onMount, onCleanup } from 'solid-js'
import { useParams } from '@solidjs/router'
import { fetchPublicProfile } from '../api/client'
import { DignitasSparkline } from '../components/DignitasSparkline'

function formatMemberSince(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'long' })
}

export default function PublicProfile() {
  const params = useParams<{ handle: string }>()
  const [profile] = createResource(() => params.handle, fetchPublicProfile)

  onMount(() => {
    document.title = `Testudo — /d/${params.handle}`
    let meta = document.querySelector<HTMLMetaElement>('meta[name="robots"]')
    if (!meta) {
      meta = document.createElement('meta')
      meta.setAttribute('name', 'robots')
      document.head.appendChild(meta)
    }
    meta.content = 'noindex, nofollow'
  })

  createEffect(() => {
    const p = profile()
    if (!p) return
    const meta = document.querySelector<HTMLMetaElement>('meta[name="robots"]')
    if (meta && p.allow_indexing) {
      meta.content = 'index, follow'
    }
  })

  onCleanup(() => {
    document.title = 'Testudo'
    const meta = document.querySelector<HTMLMetaElement>('meta[name="robots"]')
    if (meta) meta.content = 'noindex, nofollow'
  })

  return (
    <div class="min-h-screen bg-main-bg text-text-primary flex flex-col">
      <header class="border-b border-container-border px-6 md:px-8 py-4 shrink-0 bg-main-bg">
        <div class="flex items-center gap-3">
          <a href="https://testudo.vip" class="flex items-center gap-2 hover:opacity-80 transition-opacity">
            <img src={import.meta.env.BASE_URL + 'shield.svg'} alt="Testudo" class="w-5 h-5 object-contain opacity-60" />
            <span class="font-mono text-lg tracking-widest text-text-primary">TESTUDO</span>
          </a>
          <span class="font-mono text-[10px] tracking-widest text-text-tertiary">/ PUBLIC PROFILE</span>
        </div>
      </header>

      <main class="flex-1 flex flex-col items-center px-6 py-12">
        <Show when={!profile.loading} fallback={<LoadingState />}>
          <Show when={profile()} fallback={<NotFoundState handle={params.handle} />}>
            {(p) => (
              <div class="w-full max-w-lg space-y-4">
                <div class="border border-container-border bg-container-bg">
                  <div class="px-6 py-5">
                    <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                      // TESTUDO_PROFILE
                    </div>
                    <h1 class="font-mono text-2xl font-bold text-text-primary tracking-wider">
                      {p().handle}
                    </h1>
                    <div class="font-mono text-[10px] text-text-tertiary mt-1">
                      member since {formatMemberSince(p().member_since)}
                    </div>
                    <Show when={p().bio}>
                      <p class="font-mono text-sm text-text-secondary mt-3 leading-relaxed">{p().bio}</p>
                    </Show>
                  </div>
                </div>

                <Show when={p().score !== null}>
                  <div class="border border-container-border bg-container-bg">
                    <div class="px-6 py-5">
                      <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-3">
                        // DIGNITAS_SCORE
                      </div>
                      <div class="font-mono text-4xl font-bold text-text-primary">
                        {parseFloat(p().score!).toFixed(1)}
                      </div>
                      <div class="font-mono text-[10px] text-text-tertiary mt-1">/100</div>
                    </div>
                  </div>
                </Show>

                <Show when={p().sparkline !== null}>
                  <div class="border border-container-border bg-container-bg">
                    <div class="px-6 pt-5 pb-3">
                      <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-3">
                        // 90D_HISTORY
                      </div>
                      <DignitasSparkline snapshots={p().sparkline!} />
                      <div class="font-mono text-[10px] text-text-tertiary text-right mt-1">90d</div>
                    </div>
                  </div>
                </Show>

                <Show when={p().streak_days !== null}>
                  <div class="border border-container-border bg-container-bg">
                    <div class="px-6 py-5">
                      <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-3">
                        // DISCIPLINE_STREAK
                      </div>
                      <div class="flex items-baseline justify-between">
                        <div>
                          <span class="font-mono text-3xl font-bold text-text-primary">{p().streak_days}</span>
                          <span class="font-mono text-sm text-text-tertiary ml-1">days clean</span>
                        </div>
                        <div class="font-mono text-[10px] text-text-tertiary">
                          LONGEST {p().longest_ever}d
                        </div>
                      </div>
                    </div>
                  </div>
                </Show>
              </div>
            )}
          </Show>
        </Show>
      </main>

      <footer class="border-t border-container-border px-8 py-4 shrink-0">
        <a
          href="https://testudo.vip"
          class="font-mono text-[10px] text-text-tertiary hover:text-text-secondary transition-colors"
        >
          testudo.vip
        </a>
      </footer>
    </div>
  )
}

function LoadingState() {
  return (
    <div class="flex flex-col items-center gap-4 py-24">
      <div class="w-4 h-4 border-2 border-text-secondary border-t-text-primary rounded-full animate-spin" />
      <p class="font-mono text-xs text-text-tertiary tracking-wider">LOADING...</p>
    </div>
  )
}

function NotFoundState(props: { handle: string }) {
  return (
    <div class="w-full max-w-lg">
      <div class="border border-container-border bg-container-bg px-6 py-10 text-center">
        <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-4">// 404</div>
        <p class="font-mono text-sm text-text-secondary">
          <span class="text-text-primary">/{props.handle}</span> not found
        </p>
        <p class="font-mono text-[10px] text-text-tertiary mt-2">this handle hasn't been claimed</p>
      </div>
    </div>
  )
}
