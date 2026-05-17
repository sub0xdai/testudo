
Balance Discrepancy: Extension vs Desk

  Problem: Extension popup shows ~$70, desk Account page shows ~$74.56 for the same Bybit account.

  Root cause: They display different balance fields from the same CCXT API response.

  - Desk (ExchangeCard.tsx:140): Shows primary.total — includes available + locked + unrealized PnL
  - Extension (MainView.tsx:61-65): Computes available() + locked() — missing unrealized PnL component

  Fix options (pick one):

  1. Extension uses total field — Change MainView.tsx line 61-65 to use usdt().total instead of computing available + locked. Keep the available/locked breakdown as-is for the detail line.
  2. Desk uses available + locked — Change ExchangeCard.tsx:140 to compute available + locked instead of using total. Less accurate but matches extension.

  Recommended: Option 1. The total field is the authoritative balance from the exchange.

  Files:
  - Extension: testudo-extension/src/popup/components/MainView.tsx lines 58-66
  - Desk: testudo-journal/src/components/account/ExchangeCard.tsx line 140
  - Types: Check BalanceResponse in testudo-extension/src/types.ts — confirm it has a total field alongside available and locked
