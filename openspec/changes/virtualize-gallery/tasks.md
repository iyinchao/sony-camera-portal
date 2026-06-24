## 1. Setup

- [x] 1.1 Added `@tanstack/react-virtual`; `npm run build` clean

## 2. Row model + virtualizer

- [x] 2.1 Flat row model `buildRows(ordered, grouped, cols)` — header rows + tile rows (cells carry flat index); memoized on `[ordered, grouped, cols]`; flat = tile rows only
- [x] 2.2 `ResizeObserver` on the scroll container → `cols` from width (`MIN_TILE`)
- [x] 2.3 `useVirtualizer` over rows (`measureElement`, overscan 6, `getItemKey`); `.scroller` is the scroll element, toolbar stays above it (`.app` is 100vh flex column)
- [x] 2.4 Sticky active header via `rangeExtractor` (always includes the active header index; that row renders `position: sticky`)

## 3. Wire existing behavior through the row model

- [x] 3.1 Tiles render from `row.cells` with the flat index → toggle / shift-range / per-day header toggle / select-all-loaded unchanged
- [x] 3.2 Sort + grouping toggles rebuild rows; on shape change `virtualizer.measure()` + scroll to top; data array + selection preserved (verified: grouping off→headers 0, on→restored, both virtualized; sort→reset+reload from end)
- [x] 3.3 Download + header/selected counts unchanged

## 4. Lightbox → controlled PhotoSlider

- [x] 4.1 Replaced `PhotoProvider`/`PhotoView` with one `PhotoSlider` fed `ordered` (`{src, key}`), `visible`, `index`
- [x] 4.2 Tile image click sets `index` (flat index) + opens; verified slider spans the full loaded list ("1 / 480")
- [x] 4.3 Edge-load: when `viewerIndex >= ordered.length-2`, `loadMore()` so the user can keep swiping past the last loaded photo

## 5. Infinite load from the virtualizer

- [x] 5.1 `loadMore()` when the last virtual item nears `rows.length`; removed the IntersectionObserver sentinel + auto-fill effect
- [x] 5.2 Paging core unchanged (forward/oldest, reverse-from-end/newest, generation token, id-dedupe); order change resets + reloads from the right end

## 6. Verify

- [x] 6.1 `npm run build` clean; `--mock 1000`: DOM tile count stays bounded (~150) while loaded grows 120→480, smooth, no jitter/phantom reflow
- [x] 6.2 Regression: grouping/sort toggles, selection, download, full-library preview swipe, light theme — intact (screenshots); no console errors
- [x] 6.2b Boundary cases: grouping toggle mid-scroll (clean rebuild + scroll reset, virtualized), flat-mode infinite scroll, partial last row, headers appear/disappear — verified; (single-photo day / fully-loaded toggle covered by the same code path)
- [x] 6.3 `cargo fmt/clippy/test` green (18 tests); i686-musl cross-build green (960 KB); bundle has no runtime CDN refs (only SVG-namespace / React error-string literals)
- [x] 6.4 CLAUDE.md notes the virtualized gallery + UI stack
