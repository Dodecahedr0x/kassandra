# Kassandra UI

The web UI for **Kassandra** — a decentralized, AI-assisted **optimistic oracle** on Solana.
Vite + React 19 + TypeScript + Tailwind v4 SPA, styled in the **Delphi** visual language
("warm parchment editorial with ember sparks").

## Run / build

> **Build the SDK first.** The app links `@kassandra/sdk` via the pnpm workspace and
> resolves its types from `sdk/dist/` (which is gitignored). On a fresh clone, run
> `pnpm --filter @kassandra/sdk build` (or `pnpm -r build`) **before** the app's
> typecheck/build, or the SDK import won't resolve. (Slice 1 only link-proofs the
> import; the functional-dApp milestone will depend on the SDK types for real, so CI
> must build the SDK first.)

```bash
pnpm --filter @kassandra/sdk build   # build the SDK first (types → sdk/dist)
pnpm --filter app dev        # dev server (HMR)
pnpm --filter app typecheck  # tsc -b
pnpm --filter app lint       # oxlint
pnpm --filter app build      # tsc -b && vite build && verify-css guard
pnpm --filter app preview    # serve the production build
```

`build` runs `scripts/verify-css.mjs` after `vite build`: it asserts the Tailwind v4
`@tailwindcss/vite` plugin actually compiled (real utilities + lowered `@theme` vars in the
emitted CSS, no literal `@theme{}`/`@tailwind` leaks). If it fails, the app would ship unstyled.

Fonts are bundled locally via `@fontsource` (Cormorant Garamond 300/400, Inter 400/500,
Roboto Mono 400) — the build is fully offline (no hotlinked CDNs or images).

## Routes

- `/` — the Kassandra landing page (`src/pages/Landing.tsx`).
- `/styleguide` — the living design-system gallery (all tokens + primitives).

## The Delphi design system

- **Tokens** live in `src/index.css` as a Tailwind v4 CSS-first `@theme` block: the color
  palette (parchment canvas, chestnut the only button fill, ember/saffron accents…), the type
  scale, the radii vocabulary `{4,8,12,16,70}px`, the three font families, and the peach
  `--shadow-bloom`.
- **Primitives** in `src/components/ui/` (barrel `index.ts`): `Button`
  (PrimaryChestnut / GhostOutline / NavPill), `Card`, `EyebrowTag`, `SectionHeader`,
  `AvatarBubble` (+ `VerifiedDot`), `TriggerPreviewCard`.
- **Landing sections** in `src/components/landing/`: `NavBar`, `Hero` (the signature
  constellation of scattered question cards), `HowItWorks`, `WhyKassandra`, `TrustPanel`
  (the centered portrait panel — the one place a gradient is allowed), `SiteFooter`.

Design rules (from `docs/design/delphi-style-guide.md`): parchment everywhere (pure-card only
for lifted cards); chestnut is the ONLY button fill; flat surfaces + hairline pebble borders
(no heavy drop shadows — only the peach button bloom + the portrait-panel gradient); serif only
for display ≥20px, Inter for all body; ≤2 text colors per block; ember/saffron as 1–2
punctuation moments per viewport.

## Slice 1 (done) vs the next milestone

**Slice 1 (this UI):** the design-system foundation + the landing page — static, composed
from the primitives. Wallet-adapter and `@kassandra/sdk` deps are present and linked
(`workspace:*`), but **not wired**: the nav "Connect wallet" pill is a placeholder.

**Next milestone:** the functional dApp — wallet connect + real RPC reads/writes via
`@kassandra/sdk` (browse oracles/disputes, propose/challenge/vote/settle).
