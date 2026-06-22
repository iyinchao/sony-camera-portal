## Why

A Sony a6000's only wireless export path is "Send to Smartphone", which forces
users into Sony's app and offers no batch download to a desktop. We want to
browse, multi-select, and download JPEGs from the camera over its own Wi-Fi AP
using just a local web page — no Sony app, no cloud, fully offline. This change
delivers the first end-to-end usable product (camera → server → browser).

## What Changes

- Initialize the Go module and the `camera/`, `server/`, `web/` package layout
  plus `main.go` CLI entrypoint.
- **camera/**: discover the camera's ContentDirectory service from
  `DmsDescPush.xml`, enumerate photos via a UPnP `Browse` SOAP action, parse each
  item's `<res>` URLs (thumbnail / original), and expose a typed photo list and
  byte-streaming fetchers. Offline unit tests against captured XML fixtures.
- **server/**: a `127.0.0.1`-only HTTP server that embeds `web/` via `go:embed`
  and serves the proxy API:
  - `GET /api/list` → JSON photo list
  - `GET /api/thumb/:id` → proxied thumbnail bytes
  - `GET /api/photo/:id` → proxied original JPEG (download)
- **web/**: a thumbnail grid with checkbox + shift-range multi-select and a
  "Download selected" flow that streams originals to the browser.
- Graceful, human-readable errors when the client is not joined to the camera AP.

## Capabilities

### New Capabilities
- `camera-client`: discover the camera's ContentDirectory service, enumerate
  photos via UPnP `Browse`, and fetch thumbnail/original image bytes over HTTP.
- `gallery-server`: a localhost-only HTTP server that embeds the web UI and
  proxies `/api/list`, `/api/thumb/:id`, `/api/photo/:id` to the camera.
- `web-gallery`: a browser UI with a thumbnail grid, multi-select, and a
  download-selected flow.

### Modified Capabilities
<!-- None — greenfield project, no existing specs. -->

## Impact

- New code: `go.mod`, `main.go`, `camera/`, `server/`, `web/`, with `testdata/`
  XML fixtures under `camera/`.
- Dependencies: standard library only (`net/http`, `encoding/xml`, `embed`).
- No runtime network calls except to the camera at `192.168.122.1`; server binds
  `127.0.0.1` only; no telemetry.
- Out of scope for this change (future): Termux `POST /api/save`, browser
  auto-open vs print-URL polish (M4), and release tooling (M5).
