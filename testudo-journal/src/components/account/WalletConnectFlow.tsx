/** @anchor ui:journal:WalletConnectFlow
 * @tags ui */

import { createSignal, Show, For, type JSX } from 'solid-js'
import { loadWallet, getLoadedWallet } from '../../config/wallet'
import { exchangeApi } from '../../api/client'

// ─── State Machine ───

type WalletFlowState =
  | { step: 'idle' }
  | { step: 'init-agent'; address: string }
  | { step: 'signing'; accountId: string; agentAddress: string; typedData: Record<string, unknown>; nonce: number }
  | { step: 'approving'; accountId: string; signature: string; nonce: number }
  | { step: 'success'; accountId: string; agentAddress: string }
  | { step: 'error'; message: string }

interface WalletConnectFlowProps {
  onComplete: () => void
  existingAccountId?: string
}

// ─── Step Progress ───

function getStepLabels(isReauth: boolean): readonly string[] {
  return isReauth
    ? ['Connect', 'Sign', 'Approve'] as const
    : ['Connect', 'Initialize', 'Sign', 'Approve'] as const
}

function stepIndex(step: WalletFlowState['step'], isReauth: boolean): number {
  if (isReauth) {
    switch (step) {
      case 'idle': return 0
      case 'init-agent': return 0
      case 'signing': return 1
      case 'approving': return 2
      case 'success': return 3
      case 'error': return -1
    }
  }
  switch (step) {
    case 'idle': return 0
    case 'init-agent': return 1
    case 'signing': return 2
    case 'approving': return 3
    case 'success': return 4
    case 'error': return -1
  }
}

function CheckIcon(props: { size: number }): JSX.Element {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={props.size} height={props.size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  )
}

// ─── Error Extraction ───

function extractErrorMessage(err: unknown): string {
  if (!err || typeof err !== 'object') return 'An error occurred'

  if ('shortMessage' in err) {
    return String((err as { shortMessage: string }).shortMessage)
  }
  // fetch-based error — body already parsed as Error message
  if ('message' in err) {
    return String((err as { message: string }).message)
  }
  return 'An error occurred'
}

// ─── Component ───

export function WalletConnectFlow(props: WalletConnectFlowProps) {
  const [state, setState] = createSignal<WalletFlowState>({ step: 'idle' })
  const isReauth = () => !!props.existingAccountId

  function getConnectedAddress(): string | undefined {
    return getLoadedWallet()?.getAddress() ?? undefined
  }

  async function startFlow() {
    const address = getConnectedAddress()
    if (!address) return

    setState({ step: 'init-agent', address })

    try {
      let account_id: string
      let agent_address: string

      if (props.existingAccountId) {
        // Re-authorize path: skip init, go straight to approve-data
        account_id = props.existingAccountId
        const approveData = await exchangeApi.getApproveData(account_id)
        agent_address = approveData.agent_address
        const { typed_data, nonce } = approveData

        setState({
          step: 'signing',
          accountId: account_id,
          agentAddress: agent_address,
          typedData: typed_data,
          nonce,
        })

        const provider = window.ethereum
        if (!provider) throw new Error('No wallet provider found')

        const signature = await (provider as { request: (args: { method: string; params: [string, string] }) => Promise<string> }).request({
          method: 'eth_signTypedData_v4',
          params: [address, JSON.stringify(typed_data)],
        })

        setState({ step: 'approving', accountId: account_id, signature, nonce })

        await exchangeApi.approveAgent(account_id, signature, nonce)
      } else {
        // Normal init path
        const initResult = await exchangeApi.initAgentWallet(address)
        account_id = initResult.account_id
        agent_address = initResult.agent_address

        const approveData = await exchangeApi.getApproveData(account_id)
        const { typed_data, nonce } = approveData

        setState({
          step: 'signing',
          accountId: account_id,
          agentAddress: agent_address,
          typedData: typed_data,
          nonce,
        })

        const provider = window.ethereum
        if (!provider) throw new Error('No wallet provider found')

        const signature = await (provider as { request: (args: { method: string; params: [string, string] }) => Promise<string> }).request({
          method: 'eth_signTypedData_v4',
          params: [address, JSON.stringify(typed_data)],
        })

        setState({ step: 'approving', accountId: account_id, signature, nonce })

        await exchangeApi.approveAgent(account_id, signature, nonce)
      }

      setState({ step: 'success', accountId: account_id, agentAddress: agent_address })

      // Notify extension content script of successful wallet connection
      window.postMessage(
        {
          type: 'TESTUDO_ACCOUNT_LINKED',
          account: { id: account_id, exchange_name: 'hyperliquid' },
        },
        window.location.origin,
      )
    } catch (err: unknown) {
      setState({ step: 'error', message: extractErrorMessage(err) })
    }
  }

  function handleRetry() {
    setState({ step: 'idle' })
  }

  // ─── Render ───

  return (
    <div class="space-y-6">
      {/* Success state */}
      <Show when={state().step === 'success'}>
        {(() => {
          const s = state() as Extract<WalletFlowState, { step: 'success' }>
          return (
            <div class="space-y-6">
              <div class="text-center py-4">
                <div class="w-12 h-12 mx-auto border-2 border-text-primary flex items-center justify-center mb-4">
                  <CheckIcon size={24} />
                </div>
                <h3 class="font-display text-lg font-bold text-text-primary mb-2">
                  {isReauth() ? 'WALLET RE-AUTHORIZED' : 'WALLET CONNECTED'}
                </h3>
                <p class="font-mono text-sm text-text-secondary">
                  Agent wallet approved and active
                </p>
                <p class="font-mono text-xs text-text-tertiary mt-2">
                  Agent: {s.agentAddress.slice(0, 6)}...{s.agentAddress.slice(-4)}
                </p>
              </div>
              <button
                onClick={props.onComplete}
                class="w-full px-8 py-4 bg-transparent btn-primary font-mono font-bold text-lg"
              >
                DONE
              </button>
            </div>
          )
        })()}
      </Show>

      {/* Error state */}
      <Show when={state().step === 'error'}>
        {(() => {
          const s = state() as Extract<WalletFlowState, { step: 'error' }>
          return (
            <div class="space-y-4">
              <div class="px-4 py-3 border border-signal-red bg-signal-red/10">
                <p class="font-mono text-sm text-signal-red">{s.message}</p>
              </div>
              <button
                onClick={handleRetry}
                class="w-full px-8 py-4 bg-transparent btn-primary font-mono font-bold text-lg"
              >
                RETRY
              </button>
            </div>
          )
        })()}
      </Show>

      {/* Idle / Processing states */}
      <Show when={state().step !== 'success' && state().step !== 'error'}>
        {(() => {
          const current = state()
          const labels = getStepLabels(isReauth())
          const idx = stepIndex(current.step, isReauth())
          const isProcessing = current.step === 'init-agent' || current.step === 'signing' || current.step === 'approving'
          const address = getConnectedAddress()

          return (
            <div class="space-y-6">
              {/* Step progress */}
              <div class="flex items-center justify-between">
                <For each={[...labels]}>
                  {(label, i) => (
                    <div class="flex items-center">
                      <div class={`w-6 h-6 flex items-center justify-center text-xs font-mono font-bold ${
                        i() < idx
                          ? 'bg-text-primary text-main-bg'
                          : i() === idx
                            ? 'border-2 border-text-primary text-text-primary'
                            : 'border border-container-border text-text-tertiary'
                      }`}>
                        <Show when={i() < idx} fallback={<>{i() + 1}</>}>
                          <CheckIcon size={12} />
                        </Show>
                      </div>
                      <span class={`ml-1.5 font-mono text-xs ${
                        i() <= idx ? 'text-text-secondary' : 'text-text-tertiary'
                      }`}>
                        {label}
                      </span>
                      <Show when={i() < labels.length - 1}>
                        <div class={`w-8 h-px mx-2 ${
                          i() < idx ? 'bg-text-primary' : 'bg-container-border'
                        }`} />
                      </Show>
                    </div>
                  )}
                </For>
              </div>

              {/* Wallet info and action */}
              <Show when={!address}>
                <div class="space-y-4">
                  <p class="font-mono text-sm text-text-secondary">
                    {isReauth()
                      ? 'Connect your wallet to re-authorize the agent keypair.'
                      : 'Connect your Ethereum wallet to authorize an agent keypair for Hyperliquid trading.'}
                  </p>
                  <button
                    onClick={() => loadWallet().then(k => k.open())}
                    class="w-full px-8 py-4 bg-transparent btn-primary font-mono font-bold text-lg"
                  >
                    CONNECT WALLET
                  </button>
                </div>
              </Show>

              <Show when={address}>
                <div class="space-y-4">
                  <div class="flex items-center justify-between px-4 py-3 border border-container-border bg-main-bg">
                    <div>
                      <p class="font-mono text-xs text-text-tertiary">Connected Wallet</p>
                      <p class="font-mono text-sm text-text-primary">
                        {address!.slice(0, 6)}...{address!.slice(-4)}
                      </p>
                    </div>
                    <button
                      onClick={() => loadWallet().then(k => { k.disconnect(); setState({ step: 'idle' }) })}
                      class="px-3 py-1 font-mono text-xs text-text-tertiary border border-container-border hover:text-text-primary hover:border-text-primary/30 transition-colors"
                    >
                      Disconnect
                    </button>
                  </div>

                  <Show when={isProcessing} fallback={
                    <button
                      onClick={startFlow}
                      class={`w-full py-3 border font-mono font-bold text-xs tracking-wider transition-colors ${
                        isReauth()
                          ? 'border-signal-amber text-signal-amber hover:bg-signal-amber hover:text-main-bg'
                          : 'border-text-primary text-text-primary hover:bg-text-primary hover:text-main-bg animate-glow-pulse'
                      }`}
                    >
                      {isReauth() ? '[ RE-AUTHORIZE AGENT WALLET ]' : '[ AUTHORIZE AGENT WALLET ]'}
                    </button>
                  }>
                    <div class="text-center py-4">
                      <div class="inline-block w-6 h-6 border-2 border-text-primary border-t-transparent rounded-full animate-spin mb-3" />
                      <p class="font-mono text-sm text-text-secondary">
                        {current.step === 'init-agent' && (isReauth() ? 'Fetching agent data...' : 'Generating agent keypair...')}
                        {current.step === 'signing' && 'Waiting for wallet signature...'}
                        {current.step === 'approving' && 'Submitting approval to Hyperliquid...'}
                      </p>
                      <Show when={current.step === 'signing'}>
                        <p class="font-mono text-xs text-text-tertiary mt-2">
                          Check your wallet for the signing prompt
                        </p>
                      </Show>
                    </div>
                  </Show>
                </div>
              </Show>
            </div>
          )
        })()}
      </Show>
    </div>
  )
}
