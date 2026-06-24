## ADDED Requirements

### Requirement: Virtualized gallery rendering

The gallery SHALL render only the photo rows near the viewport (windowed/
virtualized), keeping the DOM size roughly constant regardless of how many photos
are loaded, while preserving date grouping, sort/group toggles, selection,
download, infinite scroll, and the photo preview.

#### Scenario: Constant DOM while scrolling a large library

- **WHEN** the user scrolls through hundreds or thousands of loaded photos
- **THEN** only the rows near the viewport are mounted (plus a small overscan),
  so the number of rendered tiles stays bounded and scrolling stays smooth
  without jitter or phantom reflow

#### Scenario: Behavior preserved under virtualization

- **WHEN** photos are virtualized
- **THEN** date grouping with sticky headers, the newest/oldest sort toggle, the
  grouping on/off toggle, per-tile / shift-range / per-day / select-all-loaded
  selection, and download all continue to work as before

#### Scenario: Responsive columns

- **WHEN** the window is resized
- **THEN** the grid recomputes its column count and re-lays-out rows accordingly,
  without losing the current selection

#### Scenario: Toggle grouping while scrolled

- **WHEN** the user turns date grouping on or off after scrolling into a large
  loaded set
- **THEN** the rows rebuild cleanly (no blank/overlapping rows or stuck scroll),
  the loaded photos and selection are preserved, and infinite loading continues
  to work against the new row layout

### Requirement: Lightbox spans the full loaded library

The photo preview SHALL let the user swipe through all loaded photos, not only
the ones currently mounted by virtualization.

#### Scenario: Swipe past mounted tiles

- **WHEN** the user opens a photo's preview and swipes to adjacent photos
- **THEN** the preview navigates across the full ordered list of loaded photos,
  even past tiles that virtualization has unmounted

#### Scenario: Swiping to the edge loads more

- **WHEN** the user swipes to the last loaded photo in the preview and more
  photos remain on the camera
- **THEN** the gallery fetches the next page and the preview lets the user keep
  swiping into the newly loaded photos (no dead end at the last loaded photo)

### Requirement: Infinite loading driven by the virtual range

The gallery SHALL fetch the next page when the virtualized scroll position nears
the end of the rendered rows, without duplicate or concurrent requests.

#### Scenario: Load near the end

- **WHEN** the user scrolls so the last rendered rows approach the end of the
  loaded set and more photos remain
- **THEN** the gallery fetches the next page and appends it; when none remain it
  stops requesting
