/** @anchor api:journal:exchange
 * @tags api */

import { fetchExchange } from './core'

export interface ExchangeInfo {
    id: string; name: string; display_name?: string; type: string
    exchange_type?: string; requires_passphrase?: boolean
    supported_features?: string[]; description?: string
    required_credentials?: string[]
}

export interface ExchangeAccount {
    id: string; exchange_name: string; account_name: string; is_active: boolean
    auth_mode: string; agent_wallet_address?: string | null
    requires_reauthorization?: boolean | null; last_used_at?: string | null
    created_at: string
}

export interface AddExchangeAccountPayload {
    exchange_name: string; account_name: string; api_key: string
    secret: string; passphrase?: string
}

export interface TestConnectionResult { success: boolean; latency_ms: number | null; error?: string }
export interface ExchangeBalanceEntry { asset: string; total: string; available: string; used: string }
export interface ExchangeBalanceResponse { balances: ExchangeBalanceEntry[] }
export interface InitAgentWalletResponse { account_id: string; agent_address: string }
export interface ApproveDataResponse { agent_address: string; typed_data: Record<string, unknown>; nonce: number }
export interface ApproveAgentResponse { success: boolean }
export interface MigrateToAgentWalletResponse { success: boolean }
export interface RevokeAgentResponse { success: boolean }

export const exchangeApi = {
    listExchanges: async () => {
        const res = await fetchExchange<{ exchanges: ExchangeInfo[] }>('')
        return res.exchanges
    },
    listAccounts: () => fetchExchange<ExchangeAccount[]>('/accounts'),
    addAccount: (payload: AddExchangeAccountPayload) =>
        fetchExchange<ExchangeAccount>('/accounts', { method: 'POST', body: JSON.stringify(payload) }),
    deleteAccount: (id: string) =>
        fetchExchange<void>(`/accounts/${id}`, { method: 'DELETE' }),
    testConnection: (id: string) =>
        fetchExchange<TestConnectionResult>(`/accounts/${id}/test`, { method: 'POST' }),
    fetchBalance: (id: string) =>
        fetchExchange<ExchangeBalanceResponse>(`/accounts/${id}/balance`),
    initAgentWallet: (walletAddress: string) =>
        fetchExchange<InitAgentWalletResponse>('/agent-wallet/init', {
            method: 'POST', body: JSON.stringify({ wallet_address: walletAddress }),
        }),
    getApproveData: (accountId: string) =>
        fetchExchange<ApproveDataResponse>('/agent-wallet/approve-data', {
            method: 'POST', body: JSON.stringify({ account_id: accountId }),
        }),
    approveAgent: (accountId: string, signature: string, nonce: number) =>
        fetchExchange<ApproveAgentResponse>('/agent-wallet/approve', {
            method: 'POST', body: JSON.stringify({ account_id: accountId, signature, nonce }),
        }),
    migrateToAgentWallet: (accountId: string, walletAddress: string) =>
        fetchExchange<MigrateToAgentWalletResponse>('/agent-wallet/migrate', {
            method: 'POST', body: JSON.stringify({ account_id: accountId, wallet_address: walletAddress }),
        }),
    revokeAgent: (id: string) =>
        fetchExchange<RevokeAgentResponse>(`/agent-wallet/${id}/revoke`, { method: 'DELETE' }),
}
