## Context

Greenfield Go project. The camera (Sony a6000) exposes a UPnP/DLNA
ContentDirectory over its own Wi-Fi AP at a fixed `192.168.122.1`. A browser
cannot talk to it directly (no CORS headers, mixed-content block, no internet on
the AP), so we interpose a localhost Go server that fetches server-side and
serves an embedded UI. This change builds the three layers (camera client,
server, web) into one shippable end-to-end MVP.

## Goals / Non-Goals

**Goals:**
- A typed, offline-testable camera client (discovery + Browse + image fetch).
- A `127.0.0.1`-only server that embeds the UI and proxies list/thumb/photo.
- A minimal but usable browser gallery: grid, multi-select, download.
- Standard library only; `gofmt`/`go vet` clean; table-driven tests.

**Non-Goals:**
- Termux `POST /api/save` and browser auto-open polish (M4).
- Release tooling: GoReleaser, GitHub Actions, install scripts (M5).
- SSDP discovery (address is fixed in AP mode; optional future fallback).
- Remote shooting, RAW transfer, infrastructure-mode PTP/IP.

## Decisions

- **Skip SSDP; hit known endpoints directly.** In AP mode the address is fixed,
  so we GET `DmsDescPush.xml` and parse the control URL. Alternative (SSDP
  M-SEARCH) is less reliable on the AP and adds multicast complexity; keep it as
  a documented future fallback, not in this change.
- **Parse XML with `encoding/xml`, pin to fixtures.** `<res>` URL shapes and
  ports vary by firmware. We capture real `DmsDescPush.xml` and `Browse`
  responses into `camera/testdata/` and unit-test parsing against them, instead
  of trusting a single assumed schema.
- **Opaque, stable photo `id`.** Use the DIDL item `@id` (UPnP object id). The
  server keeps an in-memory map `id → {thumbURL, fullURL, name}` from the last
  list call so proxy routes never expose camera URLs to the browser.
- **Proxy, don't redirect.** `/api/thumb/:id` and `/api/photo/:id` stream bytes
  server-side (`io.Copy`) so the browser stays same-origin and never needs to
  reach `192.168.122.1` (which it often cannot from an HTTPS context). Chosen
  over 302-redirect-to-camera, which would reintroduce mixed-content/CORS issues.
- **`<res>` role selection by attributes.** Pick thumbnail vs original by
  `protocolInfo` / `resolution` rather than array position, since ordering is not
  guaranteed across firmware.
- **Embed UI via `go:embed`.** Single binary, no runtime asset files. The web
  layer is **React + Vite + TypeScript**, built to static assets in `web/dist/`
  which `server/` embeds. The npm build is build-time only; the bundle is fully
  local (Vite inlines deps, no CDN), so offline use is preserved. Chosen over
  vanilla JS because the user wants a richer UX (selection state, shift-range,
  future virtualization); the cost is an added build step wired into GoReleaser
  (M5) via a `before` hook. To keep `go build`/tests working without a prior npm
  run, `web/dist/` carries a committed placeholder `index.html`, overwritten by
  the real Vite build.
- **Camera host is a flag/field defaulting to `192.168.122.1`.** Configurable for
  testing and firmware quirks without changing code.

## Risks / Trade-offs

- [Firmware variance in `<res>` URLs/ports] → Pin parsing to captured fixtures;
  make host/port configurable; keep role-selection attribute-driven.
- [Browser bulk download UX: many files trigger per-file prompts] → MVP issues
  per-photo `/api/photo/:id` downloads; a server-side zip stream is a possible
  later enhancement, out of scope here.
- [No live camera in CI] → All parsing is fixture-tested offline; live behavior
  validated manually against a real a6000 (see SPEC open questions).
- [In-memory id map staleness] → Acceptable for MVP; the map is rebuilt on each
  `/api/list`. A missing id returns 404.

## Open Questions

- Exact `<res>` URL shapes, ports, and `protocolInfo` strings on this specific
  a6000 firmware — confirm against captured `DmsDescPush.xml` / `Browse` output.
- Confirm the ContentDirectory control URL and the precise `Browse` SOAP envelope
  accepted by this camera body.
