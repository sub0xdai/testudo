# Quality Checklist: EXT-12-popup-balance-sizing

> Spec ID: EXT-12-popup-balance-sizing
> Date: 2026-02-11

## Code Quality
- [ ] No TypeScript errors (`bun run build`)
- [ ] Build succeeds for Chrome and Firefox
- [ ] No unused imports or dead code
- [ ] Balance section has `data-testid` attributes

## Sizing Compliance
- [ ] Popup width is 460px (inspect `App.tsx` root div)
- [ ] Section headers are 13px (`text-[13px]`)
- [ ] Field labels are 14px (`text-sm`)
- [ ] Input base font is 15px (in `popup.css`)
- [ ] Toggle buttons are 12px (`text-xs`)
- [ ] Status bar text is 13px
- [ ] Footer text is 13px
- [ ] No horizontal overflow at new width

## Balance Section
- [ ] Available USDT displays with green accent
- [ ] Locked USDT displays with orange accent
- [ ] Values formatted with 2 decimal places and comma separators
- [ ] "..." shown during loading
- [ ] "unavailable" shown on fetch error (non-blocking)
- [ ] Balance refreshes on popup open (onMount)
- [ ] Balance refreshes on WS_ORDER_UPDATE

## No Regressions
- [ ] Trade management inputs still save and persist
- [ ] Active orders still load and display
- [ ] Mode toggle still works (PAPER/LIVE)
- [ ] Settings view still accessible via gear icon
- [ ] Auth gate still blocks on fresh install
- [ ] `background.ts` is completely unmodified
