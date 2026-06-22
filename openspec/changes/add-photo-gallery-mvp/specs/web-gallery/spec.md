## ADDED Requirements

### Requirement: Thumbnail grid

The web UI SHALL load the photo list from `GET /api/list` and render a grid of
thumbnails, each showing the photo via `/api/thumb/:id` and its filename/date
when available.

#### Scenario: Grid renders on load

- **WHEN** the page loads and `/api/list` returns photos
- **THEN** the UI renders one thumbnail tile per photo using its `thumbUrl`

#### Scenario: Empty or error state

- **WHEN** `/api/list` returns an error or an empty list
- **THEN** the UI shows a readable message (e.g. "connect to the camera Wi-Fi" or
  "no photos found") instead of a blank screen

### Requirement: Multi-select

The web UI SHALL let the user select multiple photos using per-tile checkboxes
and SHALL support shift-click to select a contiguous range. A visible count of
selected photos SHALL be shown.

#### Scenario: Checkbox toggles selection

- **WHEN** the user clicks a tile's checkbox
- **THEN** that photo's selected state toggles and the selected count updates

#### Scenario: Shift-range selection

- **WHEN** the user selects one tile, then shift-clicks another
- **THEN** every tile between them (inclusive) becomes selected

### Requirement: Download selected

The web UI SHALL provide a "Download selected" action that downloads each
selected photo's original via `/api/photo/:id`.

#### Scenario: Download triggers per-photo originals

- **WHEN** the user has selected photos and clicks "Download selected"
- **THEN** the browser downloads each selected photo's original JPEG via its
  `/api/photo/:id` route

#### Scenario: Nothing selected

- **WHEN** no photos are selected and the user clicks "Download selected"
- **THEN** the action is a no-op (or disabled) and no request is made
