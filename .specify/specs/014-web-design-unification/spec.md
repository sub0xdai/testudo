# 014 — Web Design Unification

**Status:** Complete (Phase A)
**Phase:** 10 — Visual Cohesion
**Scope:** testudo-web (LoginPage, RegisterPage, LandingPage, Tailwind config)

## Problem

The web app uses a raw "hacker terminal" aesthetic (neon green `#00FF41`, zero border-radius, heavy monospace everywhere). The extension popup was recently refined to a premium dark-mode institutional fintech look — muted steel/slate accents, rounded corners, clean sans-serif inputs. The two products now look like they belong to different brands.

## Goal

Unify the web app's visual language with the extension's V3 aesthetic:
- **Muted steel palette** replacing neon green
- **Rounded brutalism** (rounded-md) replacing sharp zero-radius
- **Clean typography** — sans-serif for inputs/body, mono reserved for labels and data
- **Institutional fintech tone** — premium, restrained, trustworthy

## Scope

### Phase A: Auth Pages (this PR)
1. **RegisterPage** — full restyle to match extension auth screen
2. **LoginPage** — same treatment for consistency
3. **Tailwind config** — add `accent-steel` color, restore border-radius values
4. **Card component** — add `rounded-lg` support

### Phase B: Landing Page (follow-up)
1. Hero section — replace green CTAs with steel accent
2. All section components — typography and color pass
3. Footer — updated link colors
4. Global CSS — selection color, body font updates

## Design Tokens

| Token | Old | New |
|-------|-----|-----|
| Primary accent | `#00FF41` (signal-green) | `#94A3B8` (accent-steel) |
| Accent hover | `#FFFFFF` | `#CBD5E1` |
| Input bg | `main-bg` | `#18181B` (zinc-900) |
| Input border | `container-border` | `#3F3F46` (zinc-700) |
| Input focus | green ring | steel glow `0 0 0 3px rgba(148,163,184,0.15)` |
| Border radius | `0` everywhere | `0.375rem` (rounded-md) default |
| Error style | raw red mono | `text-red-400 text-sm` with border box |
| Body font | Space Mono everywhere | Sans-serif for inputs, mono for labels/data |

## RegisterPage Specifics

- Eye icon toggle on both Password and Confirm Password fields
- Labels: `text-xs font-semibold tracking-widest text-gray-400 uppercase`
- Submit button: `bg-accent-steel text-main-bg font-bold tracking-wider rounded-md`
- Error: bordered box with `border-red-500/20 bg-red-500/5 text-red-400`
- Footer link: steel accent color, not green
- Remove `font-mono` from input fields

## LoginPage Specifics

- Same treatment as RegisterPage (minus confirm password)
- Eye icon toggle on password field
- Add "FORGOT?" link (right-aligned above password, links to `/forgot-password`)

## Files Modified

- `tailwind.config.js` — colors, border-radius
- `src/index.css` — selection color, base font
- `src/pages/RegisterPage.tsx` — full restyle
- `src/pages/LoginPage.tsx` — full restyle
- `src/components/ui/Card.tsx` — rounded variant support

## Verification

- [x] RegisterPage matches extension auth screen aesthetic
- [x] LoginPage matches RegisterPage
- [x] No neon green remaining on auth pages
- [x] Eye icon toggles work on all password fields
- [x] Error states render correctly
- [x] Card borders have rounded-lg
- [x] `bun run build` passes
