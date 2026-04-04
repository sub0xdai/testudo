import { createSignal, onMount, onCleanup, Show } from 'solid-js'
import { useAuth } from '../context/AuthContext'
import { pairExtension, checkPairStatus } from '../api/client'

const CODE_TTL_SECONDS = 60

const CHROME_STORE_URL = 'https://chromewebstore.google.com'
const FIREFOX_STORE_URL = 'https://addons.mozilla.org/en-US/firefox/addon/testudo-sniper/'

// Inline SVG icons
function ShieldIcon() {
  return (
    <svg viewBox="0 0 100 100" class="w-10 h-10 text-text-tertiary" fill="currentColor">
      <circle cx="49.99989" cy="50" r="3.4386"/>
      <path d="M68.98944,4.94647q-9.4921-.31289-18.98962-.31292-9.496,0-18.9895.31292c-4.90588.16887-7.54517,2.88739-7.54535,7.67473V87.37881c.00012,4.78734,2.63947,7.506,7.54535,7.67473q9.4921.31289,18.9895.31292,9.49622,0,18.98962-.31292c4.90588-.169,7.54523-2.88739,7.54535-7.67473V12.62119C76.53461,7.83391,73.89526,5.11533,68.98944,4.94647ZM71.131,55.85217c0,5.95032-4.13934,13.32355-2.71643,20.05a17.96576,17.96576,0,0,1-5.17419-6.46771c-1.55225-3.75128,2.32837-7.632,1.29352-10.34839-.78412-2.05835-4.16357-2.572-6.68994-2.73181v1.00751a2.48048,2.48048,0,0,1-2.47766,2.47766h-.254l5.19983,7.243.06085.08478-.0484.09247-4.86249,9.296L62.85138,86.977l.27252-.18585.23431-.15985.02948.2821.09387.89716.09381.89716.03027.28967-.26678-.11707-.85748-.37628-.85748-.37628-.27979-.12286.25238-.17212.26324-.1795L54.11792,76.73444l-.06-.08459.0481-.09186,4.86084-9.29138-5.33209-7.42719H50.713V87.53265h.65277l-.15485.25677-.47046.78009-.47046.78-.145.24042-.145-.24042-.4704-.78-.47052-.78009-.15491-.25677h.629V59.83942H46.61517l-5.332,7.42719L46.144,76.558l.048.09186-.05994.08459L38.39026,87.65326l.26318.1795.25244.17212-.27979.1228-.85748.37634-.85754.37634-.26678.117.03033-.28967.09381-.89716.09381-.89716.02954-.2821.23431.15985.27246.18585,7.38928-10.42133-4.86249-9.296-.04834-.09247.06085-.08478,5.19977-7.243h-.50421a2.48048,2.48048,0,0,1-2.47766-2.47766V56.35425c-2.52637.15979-5.90582.67346-6.68994,2.73181-1.03485,2.71643,2.84576,6.59711,1.29352,10.34839a17.96576,17.96576,0,0,1-5.17419,6.46771c1.42291-6.72644-2.71643-14.09967-2.71643-20.05,0-4.85663,8.10565-5.12775,13.287-3.51636v-4.709c-5.1814,1.61139-13.287,1.34027-13.287-3.51636,0-5.95032,4.13934-13.32355,2.71643-20.05a17.96576,17.96576,0,0,1,5.17419,6.46771c1.55225,3.75128-2.32837,7.632-1.29352,10.34839.78412,2.05835,4.16357,2.572,6.68994,2.73181v-.97009a2.48051,2.48051,0,0,1,2.47766-2.47766h.50421l-5.19977-7.243-.06085-.08478.04834-.09253,4.86249-9.296L37.39856,13.023l-.27246.18585-.23431.15979-.02954-.2821-.09381-.89716-.09381-.89709-.03033-.28979.26678.11713.85754.37628.85748.37634.27979.12274-.25244.17218-.26318.1795L46.13214,23.2655l.05994.08459-.048.09186-4.8609,9.29138,5.33209,7.42725h2.89789V12.46729h-.629l.15491-.25677.47052-.78009.4704-.78.145-.24036.145.24036.47046.78.47046.78009.15485.25677H50.713v27.6933H53.6347l5.33215-7.42725L54.106,23.442l-.0481-.09186.06-.08459,7.74182-10.91882-.26324-.1795-.25238-.17218.27979-.12274.85748-.37634.85748-.37628.26678-.11713-.03027.28979-.09381.89709-.09387.89716-.02948.2821-.23431-.15979-.27252-.18585L55.46216,23.44427l4.86249,9.296.0484.09253-.06085.08478-5.19983,7.243h.254A2.48051,2.48051,0,0,1,57.844,42.63824v.97009c2.52637-.15979,5.90582-.67346,6.68994-2.73181,1.03485-2.71643-2.84576-6.59711-1.29352-10.34839a17.96576,17.96576,0,0,1,5.17419-6.46771C66.9917,30.78687,71.131,38.1601,71.131,44.11041c0,4.85663-8.10565,5.12775-13.287,3.51636v4.709C63.02539,50.72443,71.131,50.99554,71.131,55.85217Z"/>
      <path d="M49.99976,97.36609c-6.34375,0-12.75488-.10547-19.05518-.31348-6.02441-.207-9.47949-3.7334-9.47949-9.67383V12.621c0-5.94043,3.4541-9.46631,9.47705-9.67285,12.647-.41895,25.47168-.41895,38.11328,0h.00293c6.02246.20654,9.47656,3.73242,9.47656,9.67285V87.37878c0,5.94043-3.4541,9.4668-9.47656,9.67383C62.75366,97.26062,56.34253,97.36609,49.99976,97.36609Zm.001-93.73193c-6.35645,0-12.71289.10449-19.0249.313-5.40869.186-8.51074,3.34717-8.51074,8.67383V87.37878c0,5.32715,3.103,8.48828,8.51318,8.67383,12.57666.416,25.46094.416,38.0459,0,5.4082-.18555,8.51074-3.34668,8.51074-8.67383V12.621c0-5.32617-3.10254-8.4873-8.51074-8.67383h0C62.71265,3.73865,56.3562,3.63416,50.00073,3.63416Z"/>
      <path d="M55.05017,41.20563H44.94958a1.75392,1.75392,0,0,0-1.74878,1.74872V57.04559a1.754,1.754,0,0,0,1.74878,1.74878H55.05017a1.75392,1.75392,0,0,0,1.74872-1.74878V42.95435A1.75386,1.75386,0,0,0,55.05017,41.20563Zm-9.50092,1.32452a.91635.91635,0,1,1-.91638.91638A.91637.91637,0,0,1,45.54926,42.53015Zm0,15.12109a.91635.91635,0,1,1,.91632-.91638A.91638.91638,0,0,1,45.54926,57.65125Zm4.45062-3.307a4.46185,4.46185,0,1,1,4.46191-4.46185A4.46688,4.46688,0,0,1,49.99988,54.34424Zm4.45062,3.307a.91635.91635,0,1,1,.91632-.91638A.91633.91633,0,0,1,54.4505,57.65125Zm0-13.28839a.91635.91635,0,1,1,.91632-.91632A.91632.91632,0,0,1,54.4505,44.36285Z"/>
    </svg>
  )
}

export default function Pair() {
  const auth = useAuth()
  const [code, setCode] = createSignal<string | null>(null)
  const [countdown, setCountdown] = createSignal(0)
  const [generating, setGenerating] = createSignal(false)
  const [error, setError] = createSignal('')
  const [copied, setCopied] = createSignal(false)
  const [expired, setExpired] = createSignal(false)
  const [paired, setPaired] = createSignal(false)
  let timer: ReturnType<typeof setInterval> | null = null
  let copyTimeout: ReturnType<typeof setTimeout> | null = null
  let pollTimer: ReturnType<typeof setInterval> | null = null

  onMount(() => {
    // Auto-generate code if already authenticated
    if (auth.isAuthenticated()) {
      generateCode()
    }
  })

  onCleanup(() => {
    if (timer) clearInterval(timer)
    if (copyTimeout) clearTimeout(copyTimeout)
    if (pollTimer) clearInterval(pollTimer)
  })

  async function generateCode() {
    setGenerating(true)
    setError('')
    setExpired(false)
    try {
      const { code: newCode } = await pairExtension()
      setCode(newCode)
      setCountdown(CODE_TTL_SECONDS)

      if (timer) clearInterval(timer)
      timer = setInterval(() => {
        setCountdown((prev) => {
          if (prev <= 1) {
            if (timer) clearInterval(timer)
            timer = null
            if (pollTimer) clearInterval(pollTimer)
            pollTimer = null
            setExpired(true)
            setCode(null)
            return 0
          }
          return prev - 1
        })
      }, 1000)

      if (pollTimer) clearInterval(pollTimer)
      pollTimer = setInterval(async () => {
        try {
          const { paired: isPaired } = await checkPairStatus()
          if (isPaired) {
            setPaired(true)
            if (pollTimer) clearInterval(pollTimer)
            pollTimer = null
            if (timer) clearInterval(timer)
            timer = null
          }
        } catch { /* ignore poll errors */ }
      }, 3000)
    } catch {
      setError('Failed to generate code')
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

  // After wallet connect succeeds, auto-generate code
  function handleConnect() {
    auth.connectWallet()
    // Watch for auth state change
    const check = setInterval(() => {
      if (auth.isAuthenticated()) {
        clearInterval(check)
        generateCode()
      }
    }, 500)
    // Stop checking after 60s
    setTimeout(() => clearInterval(check), 60000)
  }

  return (
    <div class="min-h-screen text-text-primary">
      {/* Background */}
      <div class="fixed inset-0 z-0">
        <div
          class="absolute inset-0"
          style={{
            'background-image': 'url(https://testudo.vip/Roman-testudo-Trajan-column-966204074.jpg)',
            'background-size': 'cover',
            'background-position': 'center',
            'background-repeat': 'no-repeat',
          }}
        />
        <div class="absolute inset-0 bg-overlay" />
      </div>

      {/* Main card */}
      <div class="relative z-10 min-h-screen flex flex-col items-center justify-center px-6 py-12">
        <div class="border border-container-border bg-main-bg/80 backdrop-blur-md max-w-lg w-full">
          {/* Header band */}
          <div class="border-b border-container-border px-10 pt-10 pb-6 text-center">
            <h1 class="font-mono text-2xl md:text-3xl tracking-[0.3em] text-text-primary mb-1">
              TESTUDO
            </h1>
            <p class="font-mono text-[10px] tracking-widest text-text-tertiary mb-6">
              TRADING TERMINAL
            </p>
            {/* Shield emblem */}
            <div class="flex items-center justify-center gap-4">
              <div class="flex-1 h-px bg-container-border" />
              <ShieldIcon />
              <div class="flex-1 h-px bg-container-border" />
            </div>
          </div>

          {/* Content area */}
          <div class="px-10 py-8">
            {/* State 1: Not authenticated */}
            <Show when={!auth.isAuthenticated() && !paired()}>
              <div class="text-center">
                <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                  // CONNECT_EXTENSION
                </div>
                <h2 class="font-mono text-sm tracking-wider text-text-primary mb-4">
                  CONNECT WALLET
                </h2>
                <p class="font-mono text-xs text-text-secondary mb-8 leading-relaxed">
                  Connect your wallet to activate the extension.
                </p>
                <button
                  onClick={handleConnect}
                  class="px-6 py-2.5 border border-container-border text-text-secondary font-mono text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
                >
                  CONNECT WALLET
                </button>
                <div class="border-t border-container-border mt-8 pt-6">
                  <p class="font-mono text-[10px] text-text-tertiary">
                    Need the extension?{' '}
                    <a href={CHROME_STORE_URL} target="_blank" rel="noopener noreferrer" class="text-text-secondary hover:text-text-primary transition-colors underline">Chrome</a>
                    {' \u00b7 '}
                    <a href={FIREFOX_STORE_URL} target="_blank" rel="noopener noreferrer" class="text-text-secondary hover:text-text-primary transition-colors underline">Firefox</a>
                  </p>
                </div>
              </div>
            </Show>

            {/* State 2: Authenticated, not yet paired */}
            <Show when={auth.isAuthenticated() && !paired()}>
              <div class="text-center">
                <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                  // PAIR_EXTENSION
                </div>
                <h2 class="font-mono text-sm tracking-wider text-text-primary mb-4">
                  PAIRING CODE
                </h2>

                <Show when={error()}>
                  <p class="font-mono text-xs text-signal-red mb-4">{error()}</p>
                </Show>

                <Show when={generating()}>
                  <p class="font-mono text-xs text-text-secondary mb-4">Generating code...</p>
                </Show>

                <Show when={code()}>
                  <p class="font-mono text-xs text-text-secondary mb-6 leading-relaxed">
                    Enter this code in your extension popup.
                  </p>
                  <button
                    onClick={copyCode}
                    class="inline-block mb-4 cursor-pointer group"
                    title="Click to copy"
                  >
                    <div class="flex gap-3 justify-center">
                      {code()!.split('').map((digit) => (
                        <span class="font-mono text-3xl md:text-4xl tracking-widest text-text-primary group-hover:text-accent-steel transition-colors">
                          {digit}
                        </span>
                      ))}
                    </div>
                  </button>
                  <p class="font-mono text-lg text-text-tertiary mb-2">
                    {minutes()}:{seconds()}
                  </p>
                  <p class="font-mono text-[10px] text-text-tertiary mb-4">
                    {copied() ? 'Copied!' : 'Click code to copy'}
                  </p>
                </Show>

                <Show when={expired()}>
                  <p class="font-mono text-xs text-text-secondary mb-6 leading-relaxed">
                    Pairing code has expired.
                  </p>
                  <button
                    onClick={generateCode}
                    disabled={generating()}
                    class="px-6 py-2.5 border border-container-border text-text-secondary font-mono text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
                  >
                    GENERATE NEW CODE
                  </button>
                </Show>

                <Show when={!code() && !expired() && !generating()}>
                  <button
                    onClick={generateCode}
                    class="px-6 py-2.5 border border-container-border text-text-secondary font-mono text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
                  >
                    GENERATE CODE
                  </button>
                </Show>
              </div>
            </Show>

            {/* State 3: Successfully paired */}
            <Show when={paired()}>
              <div class="text-center">
                <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-2">
                  // PAIRED
                </div>
                <h2 class="font-mono text-sm tracking-wider text-text-primary mb-4">
                  ✓ EXTENSION LINKED
                </h2>
                <p class="font-mono text-xs text-text-secondary mb-8 leading-relaxed">
                  Your extension is now connected to your wallet.
                </p>
                <a
                  href="/desk/"
                  class="inline-block px-6 py-2.5 border border-container-border text-text-secondary font-mono text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
                >
                  OPEN TRADING DESK
                </a>
              </div>
            </Show>
          </div>

          {/* Bottom nav cards */}
          <div class="border-t border-container-border grid grid-cols-2">
            <a
              href="/desk/"
              class="flex flex-col items-center gap-1 px-4 py-5 border-r border-container-border text-text-tertiary hover:text-text-primary hover:bg-elevated/50 transition-colors group"
            >
              <div class="font-mono text-[10px] tracking-wider text-text-secondary group-hover:text-text-primary">TESTUDO DESK</div>
              <div class="font-mono text-[9px] text-text-tertiary">Trading dashboard</div>
            </a>
            <a
              href="https://testudo.vip"
              target="_blank"
              rel="noopener noreferrer"
              class="flex flex-col items-center gap-1 px-4 py-5 text-text-tertiary hover:text-text-primary hover:bg-elevated/50 transition-colors group"
            >
              <div class="font-mono text-[10px] tracking-wider text-text-secondary group-hover:text-text-primary">TESTUDO.VIP</div>
              <div class="font-mono text-[9px] text-text-tertiary">Learn more</div>
            </a>
          </div>
        </div>
      </div>
    </div>
  )
}
