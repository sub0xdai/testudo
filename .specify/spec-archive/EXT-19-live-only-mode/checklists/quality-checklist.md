# Quality Checklist — EXT-19 Live Only Mode

| # | Check | Status |
|---|-------|--------|
| 1 | No references to `PAPER_USER_ID` in extension src/ | [ ] |
| 2 | No references to `executionMode` in extension types | [ ] |
| 3 | No references to `paperOnly` in extension src/ | [ ] |
| 4 | ModeToggle.tsx deleted | [ ] |
| 5 | "Continue Without Account" button removed | [ ] |
| 6 | Auth required to reach main view | [ ] |
| 7 | Balance fetched via live exchange only | [ ] |
| 8 | Trade operations require JWT | [ ] |
| 9 | StatusBar shows sidecar status unconditionally | [ ] |
| 10 | Extension builds (Chrome + Firefox) | [ ] |
| 11 | Backend cargo clippy + cargo test pass | [ ] |
| 12 | Extension tests pass | [ ] |
