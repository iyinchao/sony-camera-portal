## 1. Design system foundation (Tailwind + tokens + fonts)

- [x] 1.1 Tailwind v4 (`@tailwindcss/vite`) + `clsx`/`tailwind-merge` (`cn()` in `lib/utils.ts`); `@/` path alias (tsconfig + vite + @types/node); `npm run build` clean (cva added with components in Phase 2)
- [x] 1.2 Tokens in `index.css` as CSS variables + Tailwind `@theme` (light `#FAFAFA`/slate, Electric Blue accent + gradient utility, border/card, shadow scale incl. `shadow-accent`, radius)
- [x] 1.2b Dark set under `@media (prefers-color-scheme: dark)` + `color-scheme`; verified both themes follow the OS automatically (screenshots)
- [x] 1.3 Typography = system sans-serif stack (no web fonts); existing components retargeted to the new tokens so light/dark works now

## 2. Component layer (Radix-backed, cva)

- [x] 2.1 `Button` (cva: gradient primary / outline / ghost, hover lift, focus ring), `Input`, `Card` in `components/ui/` (Badge/pill-label deferred)
- [x] 2.2 `ConnectPanel` redesigned shadcn-style: Card + gradient icon header + labeled Input + full-width Connect / Auto-discover / Cancel (Card, not yet a modal Dialog) — verified light + dark
- [x] 2.3 Tile selection → Radix `Checkbox` (rounded, accent fill + check, backdrop over thumbnails); host chip + buttons restyled (date headers still CSS)

## 3. Infinite scroll

- [ ] 3.1 `api.ts`: `fetchPage(offset, limit)`; remove the temporary `fetchPhotos` adapter
- [ ] 3.2 `Gallery`: flat `photos` + `offset`/`hasMore`/`loading`/`total`; page 0 on mount; IntersectionObserver sentinel → guarded `loadMore()`; append + re-group by date; remount/reset on reconnect (key by host)
- [ ] 3.3 Selection over loaded photos (shift-range, per-day, select-all-loaded); header shows loaded (+ total when known)

## 4. Lightbox (react-photo-view)

- [x] 4.1 Added `react-photo-view`; `PhotoProvider` wraps the date-group grid, each tile's `<img>` in `PhotoView src={fullUrl}` (thumbnail is the trigger image)
- [x] 4.2 Click image → zoom/swipe lightbox; selection moved to the interactive `Checkbox` (stopPropagation + shift-range), so preview vs select no longer conflict — confirmed working

## 5. Motion + polish

- [ ] 5.1 Entrance fade-up + hover lift (cards/buttons) per the system; pulsing accent dot; gate continuous motion on `prefers-reduced-motion` (optional `framer-motion`)
- [ ] 5.2 Responsive + accessible (WCAG AA contrast, focus rings, 44px touch targets)

## 6. Verify + docs

- [ ] 6.1 `npm run build` clean; `cargo run -- --mock 200` then in-browser: scroll loads pages, click-preview works, select/shift/per-day/download intact, change-camera resets; verify light + dark (emulate `prefers-color-scheme`) — screenshots
- [ ] 6.2 Offline check: built bundle has no external font/CDN URLs; `cargo fmt/clippy/test` + i686-musl cross-build still green
- [ ] 6.3 Update README / docs / CLAUDE for the new UI (design system, infinite scroll, lightbox)
