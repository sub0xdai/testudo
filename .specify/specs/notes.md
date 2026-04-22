- [ ] Telescope should be able to grep for functions etc

- [x] Consolidate every screen so it appears 1 ui - atm it appears as though there are 3 (modal, extension, web)
- [x] The desk doesnt appear to be capturing live data yet  
- [ ] Create a character like a helper character in a video game, which is powered by an ai model and reads the docs 
- [x] Settings button is mostly useless on extension 
- [x] UX flow form landing to web app and how auth fits in is abit janky 
- [x] the trade coonfirm modal needs to adopt the theme from popup  
- [x] The 'quick' button is next to useless 
 - [x] The account section should just be combined with the desk. The connect button is just login and logout of re the wallet. the desk is the gateway to the extension via the 6 digits, and doubles as the dashboard 
- [x] Import for woo not working  
- [ ] Next add binance and other exchanges support - test functionality
- [ ] save function not working properly on md notes  
- [x] create comprehensive docs (astro handles it)  
- [x] apply for firefox/chrome extension store  
- [ ] Break-even trigger, trailing stop, partial tp (perhaps remove that) not important for now
- [ ] the launch desk screen is empty, we could have a short description here about what to do, like a tldr
- [x] The save button on the journal doesnt save notes in journal  
- [x] pnl treemap is broken ux (charts) pnl could display on the positions card in extension 
- [ ] change pw in env on n0x 
- [ ]  The P&L display on position cards — that's a real feature gap worth closing 
- [ ] User is unsure about what to press after downloading the extension and clicking link. It should be more like a connection process from there like "connect wallet entry to desk OR read more" 




● ENG-02 drafted. Full roadmap now:

  ┌──────────────────────┬─────────────────────────────────────────────────┬─────────────────────────────────────────────────────────────┐
  │         Spec         │                     Summary                     │                            Gate                             │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ RSK-01               │ Unified Risk Hub (Account) + pulse strip        │ Ready                                                       │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ RSK-02               │ Optional setup tag at Alt+X                     │ Ready                                                       │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ RSK-03               │ Weekly AI Coach (in-app only)                   │ After RSK-01, RSK-02                                        │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ QNT-01               │ Calibrated Kelly sizing (a/b/c atomic specs)    │ After RSK-02 (shipped)                                      │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ HIST-03              │ CEX import idempotent dedup (P0, blocks QNT-01c)│ After HIST-01/02 (shipped) — ship BEFORE QNT-01c            │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ HIST-04              │ HL import partial-fill aggregation (P1)         │ After HIST-03 (shipped) — prod shows ~2-3x over-count on HL │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ ENG-01               │ Dignitas living score + public profile + streak │ After RSK-03 (streak gates on RSK-03 data)                  │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ ENG-02               │ On-chain discipline attestations (EAS on Base)  │ After ENG-01 produces streak data                           │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ ENG-03 (placeholder) │ Dignitas-gated Morpho lending market            │ When ≥200 users have attestations + treasury can seed ~$25k │
  ├──────────────────────┼─────────────────────────────────────────────────┼─────────────────────────────────────────────────────────────┤
  │ OBS-01               │ Obsidian plugin (one-way push)                  │ Any time (orthogonal)                                       │
  └──────────────────────┴─────────────────────────────────────────────────┴─────────────────────────────────────────────────────────────┘

Deferred to tomorrow morning:
  - ENG-01b AC #13 manual QA (incognito browser: claim handle, toggle visibility, /desk/d/<handle>, 404 on fake, 429 on curl-spam — 10 min with coffee)
  - ENG-01c (streak counter) — unblocked now, depends on 01a + 01b + RSK-03 (all live)
  - HIST-04 build (P1, but cosmetic; Dignitas inputs are process-based so over-counted HL doesn't corrupt scores)
