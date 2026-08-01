# Quality Checklist — AW-03 Frontend Wallet Connection

**Spec ID:** AW-03-frontend-wallet-connection
**Date:** 2026-03-16

## Implementation

- [ ] wagmi + viem + wallet modal library installed
- [ ] wagmi config targeting Arbitrum chain
- [ ] WalletConnect component with full state machine
- [ ] Idle → Connecting → InitAgent → Signing → Approving → Success/Error flow
- [ ] Error states with retry functionality
- [ ] AccountPage conditional: Hyperliquid → wallet, others → API key form
- [ ] API client methods: initAgentWallet, getApproveData, approveAgent
- [ ] Account list: truncated wallet address for agent-wallet accounts
- [ ] Extension delegation link to web app

## Verification

- [ ] `cd testudo-web && bun run build` passes
- [ ] Manual test: full wallet connection flow in browser
- [ ] Error handling: wallet rejection, network error, API failure all recoverable
