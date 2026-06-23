## Context

Sony spreads photos across ~30 date containers under a container-only spine
(`0 → PhotoRoot → grouping → [date containers] → items`). Browse pages WITHIN a
container (`StartingIndex` / `RequestedCount` / `NumberReturned` / `TotalMatches`)
but across containers you must enumerate the hierarchy. The current `list_all`
walks the whole tree before returning. Confirmed from captures: the spine is
3 browses deep; date-container item counts vary (4, 50, …). We did NOT capture
the grouping browse's container elements, so `childCount`/titles are unconfirmed
— the design must not depend on them.

## Goals / Non-Goals

**Goals:**
- First page returns after only a few Browse calls (fast first paint).
- Efficient sequential pagination (infinite scroll) without re-walking.
- Works whether or not the camera exposes `childCount`.
- Keep the existing UI working (minimal client adapter); the UI rework is separate.

**Non-Goals:**
- Random-access deep paging optimization (jumping to offset 10k) — sequential
  scroll is the target; far jumps may browse intermediate containers.
- Server-side search / sort beyond the camera's natural order.

## Decisions

- **Flat offset API.** `GET /api/list?offset=0&limit=60` →
  `{ photos: [...], total: number|null, hasMore: bool }`. Default `limit` ~60,
  `offset` 0. Breaking vs the old bare array — the client updates together.
- **Lazy leaf-container walker (Pager) in `AppState`.** State: ordered
  `leaves: Vec<{id, count: Option<usize>, items: Option<Vec<Photo>>}>`, a
  `spine_resolved` flag, and `total: Option<usize>`. Built lazily:
  1. Resolve the spine once: browse from `0`, descending nodes that return only
     child containers, until a level yields items; record that level's containers
     as the ordered leaves. (For this camera: browse `0`, `PhotoRoot`, `grouping`
     → 30 leaf IDs, ~3 calls. No leaf is browsed yet.)
  2. To serve `[offset, offset+limit)`: walk leaves in order; for each, ensure its
     items are loaded (browse that container, paging `RequestedCount`), using
     cached counts to skip whole containers before `offset`. Stop once `limit`
     collected. Cache browsed items + counts so later pages don't re-browse.
- **`childCount` is opportunistic.** `browse_children` parses container
  `childCount` when present → fills `count`/`total` cheaply (instant total). When
  absent, `count` is set after a container is browsed (from `TotalMatches`), and
  `total` stays `null` (→ `hasMore` drives infinite scroll) until all leaves seen.
- **camera stays the protocol layer.** Add
  `Camera::browse_children(id, start, count) -> BrowsePage { items, containers,
  number_returned, total_matches }`, where `containers` carry `{id, title,
  child_count}`. The walk/cache logic lives in the server's Pager (keeps camera
  stateless/cloneable). `list()` can be reimplemented as "drain all pages" for
  any remaining full-list need.
- **Pager lifecycle.** Created empty; reset on `/api/connect` (new camera ⇒ new
  tree). Behind the existing `AppState` mutex.
- **Mock paginates too.** `MockSource` slices its synthetic vector by
  offset/limit and reports a real total (it knows N).
- **Frontend is out of scope here.** `api.ts` gets a minimal adapter so the
  current UI keeps working; the infinite-scroll UI + Tailwind/Radix restyle are
  the separate `gallery-ui-rework` change.

## Risks / Trade-offs

- [Assuming the spine's last level is all leaves] → if a "leaf" browse returns
  sub-containers, splice them into the ordered list at that position and continue;
  degrade gracefully rather than assume depth.
- [Unknown total without childCount] → UI uses `hasMore` for infinite scroll and
  shows a running loaded-count; total appears once fully enumerated. Acceptable.
- [Breaking `/api/list` shape] → single client, updated in the same change; no
  external consumers.
- [Sony 503 under paging] → reuse the existing 3× retry in the Browse path.

## Open Questions

- Does the grouping Browse expose `childCount` / date titles? Unconfirmed; the
  design works either way. If present on a live run, wire `childCount` → instant
  total and consider date-titled groups (a later nicety).
