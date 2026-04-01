import { createSignal, Show } from 'solid-js'
import { AddExchangeForm } from './AddExchangeForm'
import type { ExchangeInfo, ExchangeAccount } from '../../api/client'

// ─── Types ───

interface OnboardingFlowProps {
  exchanges: ExchangeInfo[]
  onComplete: (account: ExchangeAccount) => void
}

// ─── Component ───

export function OnboardingFlow(props: OnboardingFlowProps) {
  const [success, setSuccess] = createSignal(false)

  function handleSuccess() {
    setSuccess(true)
  }

  function handleDone() {
    props.onComplete({
      id: '',
      exchange_name: '',
      account_name: '',
      is_active: true,
      auth_mode: '',
      created_at: new Date().toISOString(),
    })
  }

  // ─── Render ───

  return (
    <div class="flex items-center justify-center min-h-[60vh]">
      {/* Success state */}
      <Show when={success()}>
        <div class="border border-text-primary bg-main-bg/75 backdrop-blur-md p-10 md:p-14 max-w-lg w-full text-center">
          <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-6">
            // EXCHANGE_CONNECTED
          </div>
          <div class="w-12 h-12 mx-auto border border-signal-green flex items-center justify-center mb-4">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="text-signal-green">
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </div>
          <h3 class="font-mono text-xl font-bold text-text-primary mb-2 tracking-wider">
            CONNECTED
          </h3>
          <p class="font-mono text-xs text-text-secondary mb-8">
            Your exchange has been validated and configured.
          </p>
          <button
            onClick={handleDone}
            class="w-full py-3 border border-text-primary text-text-primary font-mono font-bold text-xs tracking-wider hover:bg-text-primary hover:text-main-bg transition-colors"
          >
            [ VIEW ACCOUNT ]
          </button>
        </div>
      </Show>

      {/* Exchange form */}
      <Show when={!success()}>
        <div class="border border-container-border bg-main-bg/75 backdrop-blur-md p-8 md:p-10 max-w-2xl w-full">
          <div class="font-mono text-[10px] tracking-widest text-text-tertiary mb-6">
            // ADD_EXCHANGE
          </div>
          <h2 class="font-mono text-xl font-bold text-text-primary mb-2 tracking-wider">
            GET STARTED
          </h2>
          <p class="font-mono text-xs text-text-secondary mb-8 leading-relaxed">
            Connect an exchange to enable trading. Credentials are encrypted with AES-256-GCM.
          </p>

          <AddExchangeForm
            exchanges={props.exchanges}
            onSuccess={handleSuccess}
          />

          <a
            href="https://testudo.vip/docs/07-exchanges"
            target="_blank"
            rel="noopener noreferrer"
            class="block mt-6 font-mono text-[10px] text-text-tertiary hover:text-text-secondary transition-colors text-center"
          >
            Exchange setup guides &rarr;
          </a>
        </div>
      </Show>
    </div>
  )
}
