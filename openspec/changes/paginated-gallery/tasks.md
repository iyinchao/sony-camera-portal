## 1. camera — single-container browse

- [x] 1.1 `BrowsePage { items, containers: Vec<Container{id,title,child_count}>, number_returned, total_matches }`; `parse_browse` reads counts + container `childCount`/`title`; `soap_browse` takes start/count
- [x] 1.2 `Camera::browse_children(container_id, start, count) -> BrowsePage`; `list()` kept (now drains via paged `browse_page`)
- [x] 1.3 Unit tests: leaf page → items + total_matches=4; synthetic container-list → child containers with childCount + date titles (10 tests green)

## 2. server — pager + paginated API

- [x] 2.1 `Pager` (`pager.rs`): lazy spine resolve → ordered leaves + per-leaf count/items cache + `total: Option`; lives in `RealCamera` (reset per connect = fresh source)
- [x] 2.2 `Source::list_page(offset, limit) -> Page{photos, total, has_more}` for RealCamera (via pager) and MockSource (slice)
- [x] 2.3 `GET /api/list?offset&limit` → `{ photos, total, hasMore }` (limit default 60, clamped); 503 when not connected
- [x] 2.4 Tests: paginates without overlap (offset 0/8), total correct, hasMore at end; disconnected 503; proxy cache merges across pages (8 tests green)

## 3. server/cli — wire-up + verify backend

- [x] 3.1 `cargo fmt`/`clippy`/`test` clean; `--mock 25` smoke: `?offset=0&limit=6`→6/total25/hasMore, `?offset=24`→1/hasMore=false
- [ ] 3.2 Confirm first-page latency bounded on a real camera (spine + first leaf only) — by design; observe browse count on-device

## 4. frontend — Tailwind + Radix setup

- [ ] 4.1 Add Tailwind v4 (`@tailwindcss/vite`) + theme (dark) + `cn()` helper (clsx + tailwind-merge); `@radix-ui/react-*` for the components used
- [ ] 4.2 Configure `@/` path alias (tsconfig + vite); `npm run build` clean

## 5. frontend — infinite scroll + restyle

- [ ] 5.1 `api.ts`: `fetchPage(offset, limit)` → `{ photos, total, hasMore }`
- [ ] 5.2 App/Gallery: flat `photos` + `offset`/`hasMore`; IntersectionObserver sentinel fetches next page and appends; header shows loaded count (+ total when known)
- [ ] 5.3 Restyle ConnectPanel (Radix Dialog), toolbar (host chip/buttons), tiles/checkbox with Tailwind + Radix; keep date-grouping + multi-select + download
- [ ] 5.4 `npm run build` clean; mock run verified in-browser (scroll loads more; select/download intact) — screenshot

## 6. docs + verify

- [ ] 6.1 Update README / docs / CLAUDE for the paginated `/api/list` shape + Tailwind/Radix
- [ ] 6.2 `cargo fmt/clippy/test` + cross-build i686-musl still green
