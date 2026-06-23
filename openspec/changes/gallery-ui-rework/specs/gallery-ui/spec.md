## ADDED Requirements

### Requirement: Infinite-scroll gallery

The gallery SHALL load the first page of photos on connect and fetch further
pages as the user scrolls toward the end, appending them, while preserving
date-grouping, multi-select, and download for the loaded photos.

#### Scenario: Scroll loads more

- **WHEN** the user scrolls near the bottom of the loaded photos and the API
  reports `hasMore=true`
- **THEN** the gallery fetches the next page and appends it without losing the
  current selection or scroll position

#### Scenario: Reaching the end

- **WHEN** the API reports `hasMore=false`
- **THEN** the gallery stops requesting more pages and shows the final loaded
  count

#### Scenario: No duplicate or concurrent requests

- **WHEN** a page request is already in flight
- **THEN** the gallery does not start another until it completes, and never
  re-requests a page it already has

### Requirement: Selection acts on loaded photos

Multi-select (per-tile, shift-range, per-day) and "select all" SHALL operate over
the currently loaded photos, and the UI SHALL make clear that selection is over
what is loaded (e.g. show loaded vs total counts).

#### Scenario: Select all loaded

- **WHEN** the user clicks "select all" with some pages loaded
- **THEN** all loaded photos become selected, and the count reflects loaded (not
  the camera's full total)

### Requirement: Theme follows the system color scheme

The UI SHALL support both light and dark themes and SHALL follow the operating
system's `prefers-color-scheme` automatically (no manual toggle required), with
the accent and all components legible (WCAG AA) in both.

#### Scenario: System is in light mode

- **WHEN** the OS color scheme is light
- **THEN** the UI renders the light theme (off-white canvas, slate text)

#### Scenario: System is in dark mode

- **WHEN** the OS color scheme is dark
- **THEN** the UI renders the dark theme (dark canvas, light text), and switches
  live if the OS setting changes

### Requirement: Click-to-preview lightbox

Clicking a photo's image SHALL open a full-size, zoomable/swipeable preview;
selecting a photo SHALL remain on a separate control (the tile checkbox) so the
two actions don't conflict.

#### Scenario: Open preview

- **WHEN** the user clicks a photo's thumbnail (not its checkbox)
- **THEN** a lightbox opens showing the larger image, with zoom and next/previous
  navigation, and closes on dismiss

#### Scenario: Select without previewing

- **WHEN** the user clicks a tile's selection checkbox
- **THEN** the photo's selection toggles and the lightbox does NOT open

### Requirement: Sort order and date grouping are toggleable

The gallery SHALL let the user switch the date sort order (newest-first or
oldest-first) and turn date grouping on or off, with clear icon controls.
Newest-first SHALL show the camera's actual newest photos first (paging from the
end when the total is known), not merely the newest of what happens to be loaded.

#### Scenario: Toggle sort order

- **WHEN** the user switches the sort order
- **THEN** the gallery reloads from the corresponding end and shows photos
  newest-first or oldest-first accordingly, without duplicates

#### Scenario: Turn off grouping

- **WHEN** the user turns off date grouping
- **THEN** the photos render as a single flat grid (no date headers) in the
  current sort order; turning it back on restores the date sections

### Requirement: Reconnect resets the gallery

When the user changes camera / reconnects, the gallery SHALL reset its paging
(first page of the new camera) rather than mixing photos from two cameras.

#### Scenario: Switch cameras

- **WHEN** the user connects to a different camera
- **THEN** the gallery clears and loads the first page of the new camera
