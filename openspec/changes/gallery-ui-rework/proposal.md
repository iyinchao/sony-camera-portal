## Why

The gallery UI is hand-written dark CSS, loads the whole library up front, and
has no way to view a photo large. Now that the backend paginates
(`paginated-gallery`), the frontend should: (1) adopt the **"Minimalist Modern"
design system** in `.dev/prompt.md` on a **Tailwind + Radix** component layer,
(2) use **infinite scroll** over the paged API, and (3) add **click-to-preview**
(lightbox) with `react-photo-view`.

## What Changes

- **Design system** (`.dev/prompt.md`): switch to the light "Minimalist Modern"
  look — off-white canvas (`#FAFAFA`), slate text, the Electric Blue accent
  gradient (`#0052FF → #4D7CFF`), pill section-labels, gradient buttons, soft
  shadows, and subtle motion. Tokens centralized as CSS variables / Tailwind
  theme. **Typography uses the system sans-serif stack** (no web fonts) — the
  design's dual-font (Calistoga/Inter/JetBrains Mono) is replaced by one system
  sans, with hierarchy via weight/size; keeps it lightweight and always offline.
- **Auto dark/light**: tokens are defined for light and overridden under
  `@media (prefers-color-scheme: dark)`, so the UI follows the OS theme
  automatically (the Electric Blue accent works in both). No manual toggle in v1.
- **Tailwind v4 + Radix**: `@tailwindcss/vite`, a `cn()` helper (`clsx` +
  `tailwind-merge`), `cva` for component variants, `@radix-ui/react-*` primitives
  (Dialog, Checkbox). `@/` path alias.
- **Infinite scroll**: replace the load-everything `fetchPhotos` with paged
  `fetchPage(offset, limit)`; flat `photos` + `offset`/`hasMore`, an
  IntersectionObserver sentinel appends pages. Preserve date-grouping,
  multi-select (shift-range + per-day), and download.
- **Lightbox**: wrap tiles in `react-photo-view` (`PhotoProvider`/`PhotoView`);
  clicking a photo opens a zoomable/swipeable preview. Selection stays on the
  checkbox so click-to-preview and click-to-select don't conflict.
- **Restyle** `App` / `Gallery` / `ConnectPanel` (connect panel → Radix Dialog)
  to the new system; optional `framer-motion` for entrance/hover motion
  (respecting `prefers-reduced-motion`).

## Capabilities

### New Capabilities
- `gallery-ui`: the infinite-scroll gallery interaction and click-to-preview
  lightbox (the visual design-system migration is non-behavioral, captured in
  design.md + tasks.md).

## Impact

- Frontend deps (build-time, bundled locally — no runtime CDN): `tailwindcss` v4
  + `@tailwindcss/vite`, `@radix-ui/react-*`, `clsx`, `tailwind-merge`, `cva`,
  `react-photo-view`, optionally `framer-motion`. No web fonts (system stack), no
  backend changes.
- Theming: `index.css` / `App.css` are replaced by the Tailwind theme + token
  variables, with light + dark sets that follow `prefers-color-scheme`.
- Preview loads a photo's `fullUrl` (the original). It can be heavy over the
  camera AP; a medium-resolution proxy route is a possible later backend nicety
  (out of scope here).
- `api.ts` gains `fetchPage`; the temporary `fetchPhotos` adapter is removed.
