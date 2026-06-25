## Why

With infinite scroll the gallery keeps every loaded tile in the DOM. After
removing `content-visibility` (it caused scroll jitter and phantom reflows), a
1000-photo card dump means ~5000 DOM nodes plus a thousand decoded thumbnails
held in memory. On this project's constrained targets — **iSH (iOS) and Termux
(Android)** — that is exactly what turns sluggish. Pagination bounds the *fetch*,
but not the rendered DOM. Real virtualization keeps the DOM constant regardless
of library size, which is the whole point of running on low-power devices.

## What Changes

- **Virtualize the grouped grid** with `@tanstack/react-virtual` over a flat
  **row model**: the grouped photos are flattened into rows — a date-header row,
  then one row per grid line of N tiles, repeated per day. Only visible rows (+
  overscan) render, so the DOM stays constant as you scroll.
- **Responsive columns**: the column count is derived from the container width
  and recomputed on resize; rows are rebuilt accordingly.
- **Sticky date headers** are preserved within the virtualized scroller.
- **Lightbox → controlled `PhotoSlider`**: clicking a tile opens react-photo-
  view's `PhotoSlider` driven by the **full ordered photo list** with an `index`,
  so swiping navigates across *all* photos (not just mounted ones). Replaces the
  `PhotoProvider`/`PhotoView` wrapping, which can't work once tiles unmount.
- **Infinite loading from the virtualizer**: when the last virtual row nears the
  end, fetch the next page. This replaces the IntersectionObserver sentinel and
  the auto-fill effect.
- **Selection, sort, grouping, download, dedupe, reverse-pagination stay on the
  full data array** — they operate on data, not the DOM, so they are unchanged.

## Capabilities

### Modified Capabilities
- `gallery-ui`: the gallery's rendering becomes virtualized (constant DOM,
  windowed rows) and the lightbox becomes a controlled slider over the full list;
  all existing gallery behavior (infinite scroll, grouping/sort toggles,
  selection, preview) is preserved.

## Impact

- Frontend dep (build-time, bundled, offline-safe): `@tanstack/react-virtual`
  (small, headless, pure JS). No backend changes.
- `Gallery.tsx` rendering refactor; lightbox switches from `PhotoProvider`/
  `PhotoView` to a controlled `PhotoSlider`.
- Removes the IntersectionObserver sentinel + auto-fill effect (load now driven
  by the virtualizer range). `content-visibility` is already removed.
- Risk surface is the grouped + responsive + sticky-header virtual layout; the
  data-side logic is untouched, which contains the blast radius.
