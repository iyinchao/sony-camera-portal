## Context

Backend pagination shipped (`/api/list?offset&limit → { photos, total, hasMore }`).
The React app uses a temporary adapter and a hand-written **dark** theme. This
change does the real frontend: adopt the `.dev/prompt.md` "Minimalist Modern"
**light** design system on Tailwind + Radix, add infinite scroll, and add a
click-to-preview lightbox. Hard constraint unchanged: **runtime is offline** (no
CDN) — so we avoid web fonts (system stack) and bundle every dep locally.

## Goals / Non-Goals

**Goals:**
- Centralize the `.dev/prompt.md` tokens (color, type, spacing, shadow, radius)
  as CSS variables + Tailwind theme; build reusable Button/Card/Dialog/Checkbox.
- Light theme, Electric Blue accent gradient, system sans-serif typography.
- Infinite scroll; click-to-preview lightbox; preserve select/download/connect.
- Offline preserved (system fonts, locally-bundled deps).
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
- **Auto light/dark via `prefers-color-scheme`.** The light tokens are the
  default `:root`; a `@media (prefers-color-scheme: dark)` block overrides them
  with a dark set (dark slate canvas `#0F172A`, elevated `#1E293B` cards, light
  text, same Electric Blue accent — tuned for contrast). Every component reads the
  CSS variables, so the whole UI follows the OS theme with no JS. Tailwind v4's
  `dark:` variant (keyed off `prefers-color-scheme`) covers the few per-theme
  utilities; set `color-scheme` so native controls/scrollbars match. No manual
  toggle in v1 (a future `data-theme` override of the same variables could add one).
- **System sans-serif typography (no web fonts).** Use the OS font stack
  (`system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif`)
  for everything; the design's Calistoga/Inter/JetBrains Mono is dropped, with
  hierarchy expressed via weight/size/letter-spacing instead of font family. No
  `@fontsource`/woff2 to bundle — smaller binary, instant render, inherently
  offline. (Section-label "mono" feel can use slightly wider tracking + uppercase
  in the system font.)
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
- [Offline regression via CDN refs] → system fonts (no web fonts at all);
  CI/build check that the bundle has no external CDN URLs.

## Open Questions

- Add a `framer-motion`-driven hero/empty state, or keep the gallery utilitarian?
  Lean utilitarian; reserve flourish for the connect dialog + brand.
- Medium-resolution preview proxy (`/api/preview/:id` using JPEG_LRG) — worth a
  small follow-up backend change for snappier previews?
