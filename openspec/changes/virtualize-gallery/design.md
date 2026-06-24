## Context

The gallery (from `gallery-ui-rework`) loads photos via paged infinite scroll,
groups them by date, supports sort/group toggles, multi-select, and a
react-photo-view lightbox, with macOS Finder-style tiles. Rendering currently
keeps all loaded tiles in the DOM. This change virtualizes the rendering so the
DOM stays constant, targeting smooth behavior on iSH/Termux with large libraries.

## Goals / Non-Goals

**Goals:**
- Constant DOM size regardless of how many photos are loaded/scrolled.
- Preserve every existing behavior: infinite scroll, date grouping, sort/group
  toggles, selection (per-tile, shift-range, per-day, select-all-loaded),
  download, and full-library lightbox swipe.
- No scroll jitter or phantom reflow (the reason `content-visibility` was removed).
- Stay offline (bundled dep) and pure-frontend (no backend change).

**Non-Goals:**
- Backend changes.
- Changing the visual design (Finder tiles, theme, toolbar) — only how rows are
  mounted.
- Virtualizing the *data* — the full ordered photo array stays in memory (needed
  for selection indices and lightbox); only DOM rows are windowed.

## Decisions

- **Library: `@tanstack/react-virtual`.** Headless, ~a few KB, pure JS, offline-
  safe. We need a custom grouped + responsive grid, which a batteries-included
  list/grid lib (e.g. virtuoso) models awkwardly; the headless virtualizer lets
  us define our own row model.
- **Flat row model.** From the ordered, grouped photos build a flat array of
  rows: `{ type: 'header', groupKey, label, count }` and
  `{ type: 'tiles', photos: Photo[], startIndex }` (one per grid line of `cols`
  tiles). `useVirtualizer({ count: rows.length, estimateSize, ... })` virtualizes
  these rows. Header rows and tile rows have different (measured) heights;
  `measureElement` handles variance.
- **Responsive columns.** A `ResizeObserver` on the scroll container computes
  `cols = floor((width - padding) / minTileWidth)`; rows are rebuilt (memoized on
  `[ordered, grouped, cols]`). `startIndex` on each tile row maps back to the flat
  ordered index for selection/shift-range/lightbox.
- **Sticky headers.** Use the virtualizer's range to render the current group's
  header as a sticky overlay at the top of the viewport (react-virtual's sticky-
  header pattern), so the active date stays pinned like today.
- **Lightbox: controlled `PhotoSlider`.** Replace `PhotoProvider`/`PhotoView`
  (which require mounted children) with a single `PhotoSlider` given
  `images = ordered.map(p => ({ src: p.fullUrl, key: p.id }))`, `visible`, and
  `index` state. A tile's image click sets `index` (its flat index) and opens the
  slider → swiping traverses the whole library, independent of what's mounted.
- **Lightbox loads more at the edge.** `PhotoSlider`'s `onIndexChange` fires as
  the user swipes; when the index nears the end of `ordered` (e.g. `>= length-2`)
  and `hasMore`, call `loadMore()`. The slider's `images` prop then grows and the
  user can keep swiping into the freshly loaded photos — so the lightbox doesn't
  "hit a wall" at the last loaded photo. This is consistent with paging direction:
  `loadMore` always appends at the end (older in newest mode), which is the same
  end the slider reaches when swiping forward. Only the end edge triggers loading
  (the start is the true newest, with nothing earlier to fetch).
- **Infinite load from the virtualizer.** When the largest virtual item index is
  within a threshold of `rows.length` (and `hasMore` / not loading / not done),
  call `loadMore()`. This replaces the IntersectionObserver sentinel and the
  auto-fill effect. The existing paging core (forward for oldest, reverse-from-end
  for newest, generation token, id-dedupe) is unchanged. The trigger is in terms
  of *rows*, but both grouped and flat row models end at the last photo's row, so
  "near the last row" maps to "near the last photo" either way (grouping only adds
  header rows, which doesn't change the end).
- **Row-model changes (grouping toggle / sort toggle / resize) and the
  virtualizer.** Rebuilding `rows` changes the count and per-index heights
  (a header row becomes a tile row, etc.), so stale measurements would misplace
  items. To keep it correct:
  - Give each row a **stable `getItemKey`** (e.g. `header:<groupKey>` /
    `tiles:<firstPhotoId>`) so reconciliation isn't index-positional.
  - On a model-shape change, **reset the measurement cache** (`virtualizer.measure()`)
    so heights are re-measured rather than reused by index.
  - **Scroll position policy:** grouping toggle keeps the same photos (no paging
    reset) but the offset is meaningless across shapes → **scroll to top** on
    grouping toggle. Sort toggle already resets paging and reloads from the other
    end → also top. (Preserving the first-visible photo across the toggle is a
    possible enhancement, not v1.)
  - The data array (`ordered`) is untouched by these toggles, so selection,
    shift-range anchor reset, and in-flight loads (guarded by the generation
    token) are unaffected — only rendering is rebuilt.
- **Scroll container.** The virtualizer needs a sized scroll element. The gallery
  body becomes the scroll parent (the sticky toolbar stays above it); the
  virtualizer measures from that element.

## Risks / Trade-offs

- [Grouped + responsive + sticky in one virtualizer is the hard case] → the flat
  row model keeps it tractable; headers are just another row type, columns are a
  pure function of width. Verify on resize and at group boundaries.
- [Variable row heights cause estimate drift] → use `measureElement` so real
  heights replace estimates; unlike `content-visibility` this corrects without
  visible jitter because react-virtual compensates scroll offset.
- [Lightbox swipe vs virtualization] → solved by the controlled `PhotoSlider`
  over the full list (actually better than today's mounted-only swipe).
- [Selection shift-range relies on contiguous indices] → preserved: rows carry
  `startIndex` into the same `ordered` array the selection logic already uses.
- [Sticky header complexity] → if the sticky overlay proves fiddly, fall back to
  non-overlay headers (each header row scrolls normally) as a smaller first step.
- [Row-model swap (grouping/sort/resize) glitches] → stable `getItemKey` +
  `virtualizer.measure()` reset + scroll-to-top on grouping toggle; verify no
  blank rows, overlap, or stuck scroll after toggling.

## Boundary conditions to verify

- A **partial last tile row** (fewer than `cols` tiles).
- **Single-photo day** (a header row followed by a 1-tile row).
- **Grouping toggled mid-load** (a page request in flight) — data array and the
  generation-guarded load must survive; only rows rebuild.
- **Grouping/sort toggled when everything is loaded** — no further loads fire.
- **Resize and a grouping toggle close together** — `cols` recompute and the row
  rebuild compose without losing selection or wedging scroll.
- **Toggle near the end vs at the top** — load trigger re-evaluates against the
  new `rows.length` and doesn't double-fire or stall.
- **Empty / not-yet-loaded** state — zero rows renders the skeleton/empty state,
  not a broken virtualizer.

## Open Questions

- Keep the macOS-Finder fixed row height assumption, or fully measure each row?
  Lean on `measureElement` for correctness; seed with an estimate (~170px).
- Overscan count — start at 4–6 rows; tune on-device for iSH.
