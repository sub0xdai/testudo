import type { AppKit } from '@reown/appkit'

let walletPromise: Promise<AppKit> | null = null
let resolvedKit: AppKit | null = null

export function loadWallet(): Promise<AppKit> {
  if (walletPromise) return walletPromise
  walletPromise = (async () => {
    const [{ createAppKit }, { EthersAdapter }, { SolanaAdapter }, networks] =
      await Promise.all([
        import('@reown/appkit'),
        import('@reown/appkit-adapter-ethers'),
        import('@reown/appkit-adapter-solana'),
        import('@reown/appkit/networks'),
      ])
    const kit = createAppKit({
      adapters: [new EthersAdapter(), new SolanaAdapter()],
      networks: [networks.mainnet, networks.arbitrum, networks.base, networks.polygon, networks.solana],
      projectId: import.meta.env.VITE_WALLETCONNECT_PROJECT_ID || '',
      metadata: {
        name: 'Testudo',
        description: 'Automated risk management for crypto trading',
        url: window.location.origin,
        icons: ['/testudo-icon.png'],
      },
      themeMode: 'dark',
    })
    resolvedKit = kit
    return kit
  })()
  return walletPromise
}

export function isWalletLoaded(): boolean {
  return resolvedKit !== null
}

export function getLoadedWallet(): AppKit | null {
  return resolvedKit
}
