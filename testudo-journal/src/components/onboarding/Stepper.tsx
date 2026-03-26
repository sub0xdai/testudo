import { Show, For, createSignal, createEffect } from 'solid-js'
import { useNavigate } from '@solidjs/router'
import { useAuth } from '../../context/AuthContext'
import { useOnboardingState } from './useOnboardingState'

const CHROME_STORE_URL = 'https://chromewebstore.google.com/detail/testudo-sniper/'

export function Stepper() {
  const { steps, activeStep, allComplete, shouldShow, dismiss } = useOnboardingState()
  const [showComplete, setShowComplete] = createSignal(false)
  const [hidden, setHidden] = createSignal(false)
  const navigate = useNavigate()
  const auth = useAuth()

  // When all steps complete, show brief message then dismiss
  createEffect(() => {
    if (allComplete() && shouldShow()) {
      setShowComplete(true)
      const timer = setTimeout(() => {
        dismiss()
        setHidden(true)
      }, 3000)
      return () => clearTimeout(timer)
    }
  })

  const handleStepClick = (index: number) => {
    switch (index) {
      case 0:
        auth.connectWallet()
        break
      case 1:
        navigate('/account')
        break
      case 2:
        // Import is auto-triggered per HIST-01 — navigate to account to add exchanges
        navigate('/account')
        break
      case 3:
        navigate('/account')
        break
    }
  }

  return (
    <Show when={shouldShow() && !hidden()}>
      <div class="relative z-10 max-w-[1400px] mx-auto px-6 pt-4">
        <Show
          when={!showComplete()}
          fallback={
            <div class="border border-signal-green/30 bg-signal-green/5 px-4 py-3 font-mono text-xs text-signal-green tracking-wider text-center">
              SETUP COMPLETE
            </div>
          }
        >
          <div class="border border-container-border bg-container-bg/50 backdrop-blur-sm px-4 py-3">
            <div class="flex items-center justify-between gap-2">
              {/* Steps */}
              <div class="flex items-center gap-1 sm:gap-2 flex-1 min-w-0">
                <For each={steps()}>
                  {(step, i) => {
                    const isComplete = () => step.complete
                    const isActive = () => i() === activeStep()
                    const isPending = () => !step.complete && i() > activeStep()

                    return (
                      <>
                        {/* Connector line (not before first step) */}
                        <Show when={i() > 0}>
                          <div
                            class="h-px flex-1 max-w-[40px] hidden sm:block transition-colors"
                            classList={{
                              'bg-signal-green/50': isComplete(),
                              'bg-container-border': !isComplete(),
                            }}
                          />
                        </Show>

                        {/* Step */}
                        <button
                          onClick={() => handleStepClick(i())}
                          class="flex items-center gap-1.5 sm:gap-2 transition-colors group shrink-0"
                          classList={{
                            'cursor-pointer': isActive() || isPending(),
                            'cursor-default': isComplete(),
                          }}
                          title={step.label}
                        >
                          {/* Circle */}
                          <div
                            class="w-5 h-5 sm:w-6 sm:h-6 flex items-center justify-center border text-[10px] sm:text-xs font-mono font-bold transition-colors shrink-0"
                            classList={{
                              'border-signal-green bg-signal-green/10 text-signal-green': isComplete(),
                              'border-text-primary text-text-primary animate-pulse': isActive(),
                              'border-container-border text-text-tertiary': isPending(),
                            }}
                          >
                            <Show when={isComplete()} fallback={i() + 1}>
                              <svg width="10" height="10" viewBox="0 0 10 10" class="text-signal-green">
                                <path d="M2 5L4 7L8 3" stroke="currentColor" stroke-width="1.5" fill="none" />
                              </svg>
                            </Show>
                          </div>

                          {/* Label (hidden on mobile) */}
                          <span
                            class="font-mono text-[10px] tracking-wider hidden md:inline whitespace-nowrap transition-colors"
                            classList={{
                              'text-signal-green': isComplete(),
                              'text-text-primary': isActive(),
                              'text-text-tertiary': isPending(),
                            }}
                          >
                            {step.label.toUpperCase()}
                          </span>
                        </button>
                      </>
                    )
                  }}
                </For>
              </div>

              {/* Extension install hint for step 4 */}
              <Show when={activeStep() === 3}>
                <a
                  href={CHROME_STORE_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  class="font-mono text-[10px] text-text-tertiary hover:text-text-secondary transition-colors whitespace-nowrap hidden sm:inline"
                >
                  Install Extension &rarr;
                </a>
              </Show>
            </div>
          </div>
        </Show>
      </div>
    </Show>
  )
}
