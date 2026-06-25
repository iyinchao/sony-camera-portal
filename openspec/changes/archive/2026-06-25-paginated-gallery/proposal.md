## Why

First paint is slow: `/api/list` enumerates the **entire** camera tree (root →
PhotoRoot → ~30 date containers → items) before returning anything, so the user
waits 30s+ on a full card. Parallelising the browse (tried earlier) doesn't fix
this — it cuts total time but the first byte still waits for the last container.
The fix is **real pagination**: fetch a small first page and return it fast, then
load more on demand.

(The frontend rework — infinite scroll + Tailwind/Radix — is split into its own
change, `gallery-ui-rework`; this change is the backend API only.)

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
- **web**: a minimal adapter only (`fetchPage` + `fetchPhotos` reads `.photos`)
  so the existing UI keeps working; the real infinite-scroll UI lands in
  `gallery-ui-rework`.

## Capabilities

### New Capabilities
- `paginated-gallery`: the paginated photo-listing API (`/api/list?offset&limit`
  → `{ photos, total, hasMore }`).

### Modified Capabilities
<!-- The visual restyle (Tailwind/Radix) and the camera/server refactor are
     implementation concerns captured in design.md + tasks.md, not new
     requirements. -->

## Impact

- API: `/api/list` response shape changes (array → object); the React client's
  `api.ts` is updated minimally to match. New query params `offset`, `limit`.
- No new dependencies (Rust or frontend).
- `childCount` in the date-container Browse is **optional**: used for an instant
  total when present, otherwise total is learned lazily as pages load (infinite
  scroll doesn't require it). No fresh camera capture needed to start.
