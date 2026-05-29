/** @anchor infra:journal-lib:useAsyncAction
 * @tags infra */

import { createSignal } from 'solid-js'

/**
 * Generic async action wrapper for UI handlers.
 *
 * Collapses the common pattern of separate `XId` + `error` signals
 * for each action into a single `pending` signal and shared error.
 */
export function useAsyncAction() {
    const [pending, setPending] = createSignal<string | null>(null)
    const [error, setError] = createSignal('')

    async function run(id: string, action: () => Promise<void>, onError?: string) {
        setPending(id)
        setError('')
        try {
            await action()
        } catch {
            setError(onError ?? 'Action failed')
        } finally {
            setPending(null)
        }
    }

    return { pending, error, setError, run }
}
