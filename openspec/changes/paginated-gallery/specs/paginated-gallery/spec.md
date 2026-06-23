## ADDED Requirements

### Requirement: Paginated photo listing

`GET /api/list` SHALL accept `offset` and `limit` query parameters and return a
page of photos as `{ photos, total, hasMore }`, where `total` MAY be null when
not yet known. The server SHALL fetch only enough of the camera tree to satisfy
the requested page (it MUST NOT enumerate the whole library to return the first
page).

#### Scenario: First page returns quickly

- **WHEN** a client requests `GET /api/list?offset=0&limit=60` against a camera
  with many photos across many containers
- **THEN** the response contains up to 60 photos and `hasMore=true`
- **AND** the server browsed only the container spine plus the first container(s)
  needed for those 60 — not every container

#### Scenario: Subsequent page continues without re-walking

- **WHEN** the client then requests `GET /api/list?offset=60&limit=60`
- **THEN** the response contains the next photos in order, with no duplicates
  from the first page

#### Scenario: Last page reports completion

- **WHEN** the client pages past the final photo
- **THEN** the response has `hasMore=false` and (by then) a numeric `total`

#### Scenario: Not connected

- **WHEN** `GET /api/list` is requested with no camera connected
- **THEN** the server returns a non-2xx status with a JSON error (the UI shows the
  connect panel)

### Requirement: Total is opportunistic, not required

The server SHALL report a numeric `total` when it can determine it cheaply (e.g.
from container `childCount`) or once all containers have been seen, and SHALL
otherwise report `total=null` while still paginating correctly via `hasMore`.

#### Scenario: Total unknown mid-scroll

- **WHEN** the camera does not expose per-container counts and not all containers
  have been loaded
- **THEN** `total` is null but `hasMore` correctly indicates more pages exist

### Requirement: Infinite-scroll gallery

The web UI SHALL load the first page on connect and fetch further pages as the
user scrolls toward the end, appending them, while preserving date-grouping,
multi-select, and download for the loaded photos.

#### Scenario: Scroll loads more

- **WHEN** the user scrolls near the bottom of the loaded photos and `hasMore` is
  true
- **THEN** the UI fetches the next page and appends it without losing the current
  selection or scroll position

#### Scenario: Reaching the end

- **WHEN** all pages are loaded (`hasMore=false`)
- **THEN** the UI stops requesting more and shows the final loaded count
