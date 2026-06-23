## Context

Backend pagination shipped (`/api/list?offset&limit → { photos, total, hasMore }`).
The React app uses a temporary adapter and a hand-written **dark** theme. This
change does the real frontend: adopt the `.dev/prompt.md` "Minimalist Modern"
**light** design system on Tailwind + Radix, add infinite scroll, and add a
click-to-preview lightbox. Hard constraint unchanged: **runtime is offline** (no
CDN) — so fonts are self-hosted.

## Goals / Non-Goals

**Goals:**
- Centralize the `.dev/prompt.md` tokens (color, type, spacing, shadow, radius)
  as CSS variables + Tailwind theme; build reusable Button/Card/Dialog/Checkbox.
- Light theme, Electric Blue accent gradient, dual-font typography.
- Infinite scroll; click-to-preview lightbox; preserve select/download/connect.
- Offline preserved (self-hosted fonts, locally-bundled deps).
- Accessible (WCAG AA contrast, focus rings, 44px touch targets, reduced-motion).

**Non-Goals:**
- The marketing-page pieces of the design system (hero graphic, pricing,
  testimonials) — we adapt the *tokens and component styles* to a gallery, not
  the landing-page layout.
- Backend changes (a medium-res preview proxy is noted as future).
- Virtualized rendering (content-visibility already handles large lists).

## Decisions

- **Design tokens from `.dev/prompt.md`.** Defined once as CSS variables and
  mirrored in the Tailwind theme: `--background:#FAFAFA`, `--foreground:#0F172A`,
  `--muted:#F1F5F9`, `--muted-foreground:#64748B`, `--accent:#0052FF`,
  `--accent-secondary:#4D7CFF`, `--border:#E2E8F0`, `--card:#FFFFFF`; the signature
  gradient `linear-gradient(135deg,#0052FF,#4D7CFF)`; shadow scale incl.
  `shadow-accent`. One source of truth; no one-off colors.
- **Self-hosted fonts (offline).** `@fontsource/inter`, `@fontsource/calistoga`,
  `@fontsource/jetbrains-mono` imported in code → Vite bundles the woff2 locally.
  Calistoga for the brand/headings, Inter for UI/body, JetBrains Mono for the
  pill section-labels. NOT the Google Fonts CDN.
- **Components: cva + tailwind-merge, Radix-backed.** Local `Button`, `Card`,
  `Badge` (section-label), Radix `Dialog` (connect), Radix `Checkbox` (tile
  select) — shadcn-style API, styled to the design system. `cn()` = `clsx` +
  `tailwind-merge`.
- **Infinite scroll.** `Gallery` owns `photos`, `offset`, `hasMore`, `loading`,
  `total`; loads page 0 on mount; an IntersectionObserver bottom sentinel calls a
  guarded `loadMore()`; appends and re-groups by `dc:date`. Keyed by host so a
  reconnect remounts with fresh paging.
- **Lightbox via `react-photo-view`.** A `PhotoProvider` at the grid root; each
  tile's image is a `PhotoView src={photo.fullUrl}`. Click the image → zoom/swipe
  preview; the selection **checkbox** (corner) stays the select affordance, so
  preview and select don't fight. Thumbnail shows immediately while the full
  loads. (Preview = original; a medium proxy is future.)
- **Motion.** Use the design system's subtle motion — entrance fade-up, hover
  lift on cards/buttons, a pulsing accent dot in the brand/section label.
  `framer-motion` for entrance/continuous; gate all continuous motion on
  `prefers-reduced-motion`. Keep it light (a gallery, not a landing page).
- **Selection across pages.** `Set<id>` over loaded photos; shift-range on the
  flat loaded index; "select all" = loaded ids, labelled to reflect loaded vs
  total.

## Risks / Trade-offs

- [Click-to-preview vs click-to-select conflict] → preview on image click, select
  on the corner checkbox; document the interaction.
- [Theme flip touches every component] → migrate token-by-token; keep the app
  building (`npm run build`/tsc) at each step.
- [framer-motion bundle weight] → only where it earns its keep; simple cases use
  CSS transitions; respect reduced-motion.
- [Preview loads the original (heavy on AP)] → acceptable for v1 with a thumb
  placeholder; flag a medium-res proxy as a follow-up.
- [Offline regression via fonts] → `@fontsource` only; CI/build check that the
  bundle has no external font/CDN URLs.

## Open Questions

- Add a `framer-motion`-driven hero/empty state, or keep the gallery utilitarian?
  Lean utilitarian; reserve flourish for the connect dialog + brand.
- Medium-resolution preview proxy (`/api/preview/:id` using JPEG_LRG) — worth a
  small follow-up backend change for snappier previews?
