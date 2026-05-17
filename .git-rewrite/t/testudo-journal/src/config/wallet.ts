import { createAppKit } from '@reown/appkit'
import { EthersAdapter } from '@reown/appkit-adapter-ethers'
import { SolanaAdapter } from '@reown/appkit-adapter-solana'
import { mainnet, arbitrum, base, polygon, solana } from '@reown/appkit/networks'

const projectId = import.meta.env.VITE_WALLETCONNECT_PROJECT_ID || ''

const ethersAdapter = new EthersAdapter()
const solanaAdapter = new SolanaAdapter()

export const appKit = createAppKit({
  adapters: [ethersAdapter, solanaAdapter],
  networks: [mainnet, arbitrum, base, polygon, solana],
  projectId,
  metadata: {
    name: 'Testudo',
    description: 'Automated risk management for crypto trading',
    url: typeof window !== 'undefined' ? window.location.origin : 'https://testudo.app',
    icons: ['/testudo-icon.png'],
  },
  themeMode: 'dark',
})
