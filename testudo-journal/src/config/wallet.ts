import { createAppKit } from '@reown/appkit'
import { EthersAdapter } from '@reown/appkit-adapter-ethers'
import { mainnet, arbitrum, base, polygon } from '@reown/appkit/networks'

const projectId = import.meta.env.VITE_WALLETCONNECT_PROJECT_ID || ''

const ethersAdapter = new EthersAdapter()

export const appKit = createAppKit({
  adapters: [ethersAdapter],
  networks: [mainnet, arbitrum, base, polygon],
  projectId,
  metadata: {
    name: 'Testudo',
    description: 'Automated risk management for crypto trading',
    url: typeof window !== 'undefined' ? window.location.origin : 'https://testudo.app',
    icons: ['/testudo-icon.png'],
  },
  themeMode: 'dark',
})
