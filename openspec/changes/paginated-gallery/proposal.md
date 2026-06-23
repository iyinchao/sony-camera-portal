## Why

First paint is slow: `/api/list` enumerates the **entire** camera tree (root →
PhotoRoot → ~30 date containers → items) before returning anything, so the user
waits 30s+ on a full card. Parallelising the browse (tried earlier) doesn't fix
this — it cuts total time but the first byte still waits for the last container.
The fix is **real pagination**: fetch a small first page and return it fast, then
load more as the user scrolls.

Separately, the UI is hand-written CSS; we want to move to **Tailwind + Radix**
for a cleaner, accessible component layer.

## What Changes

- **Paginated `/api/list`** (BREAKING response shape): `GET /api/list?offset&limit`
  returns `{ photos, total, hasMore }` instead of a bare array. A lazy
  leaf-container walker fetches only enough containers to satisfy the page (first
  page ≈ a few Browse calls, not 30+).
- **camera**: add `browse_children(container_id, start, count)` (one container
  page → items + child containers with `childCount` when present + `TotalMatches`).
- **server**: an `AppState` pager caches the ordered leaf-container list and
  per-container counts; serves `[offset, offset+limit)` across containers; resets
  on `/api/connect`. Mock source paginates too.
- **Frontend**: infinite scroll (IntersectionObserver sentinel) over the paged
  API, preserving date-grouping / multi-select / download; restyle with
  **Tailwind v4 + Radix** (connect panel → Radix Dialog, buttons/inputs/checkbox
  → Radix primitives), keeping the dark theme and offline bundling (no CDN).

## Capabilities

### New Capabilities
- `paginated-gallery`: the paginated photo-listing API and the infinite-scroll
  gallery UI behavior.

### Modified Capabilities
<!-- The visual restyle (Tailwind/Radix) and the camera/server refactor are
     implementation concerns captured in design.md + tasks.md, not new
     requirements. -->

## Impact

- API: `/api/list` response shape changes (array → object); the React client is
  updated in lockstep. New query params `offset`, `limit`.
- New deps: frontend `tailwindcss` (v4, `@tailwindcss/vite`), `@radix-ui/react-*`,
  `clsx`/`tailwind-merge`; all bundled locally (offline preserved). No new Rust
  deps.
- `childCount` in the date-container Browse is **optional**: used for an instant
  total when present, otherwise total is learned lazily as pages load (infinite
  scroll doesn't require it). No fresh camera capture needed to start.
