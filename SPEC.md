# sony-camera-portal — Project spec

## Goal
Browse, select, and download photos from a Sony a6000 over the **camera's Wi-Fi
access point**, using only a **local web UI** in the user's browser. No Sony app,
no cloud, works offline (the camera AP has no internet).

Target users run it on macOS / Windows / Linux desktops, or on an Android phone
inside Termux. **One Go codebase**, distributed as a single self-contained binary.

## Why this design (the core constraint)
A browser page hosted anywhere *other than the camera* cannot talk to the camera
directly:
- the camera's HTTP/SOAP API returns no CORS headers, so cross-origin JS fetches
  are blocked;
- an HTTPS page cannot call the `http://` camera (mixed-content block);
- on the camera AP there is no internet to load a hosted app at all.

The fix is a tiny **local server**: the browser talks to it on `localhost`
(same-origin, no CORS), and it talks to the camera **server-side** (CORS and
mixed-content rules don't apply to non-browser requests). Embedding the web UI in
the binary makes "run it, open localhost" a one-step experience on every platform.

A page served *by the camera itself* would also be same-origin — but the a6000
ships no web UI, so we supply the server ourselves.

## Camera protocol (Sony PlayMemories / Imaging Edge generation)
1. On the camera: Menu → Send to Smartphone → **Select on Smartphone**. The camera
   becomes a Wi-Fi AP and runs a UPnP/DLNA stack.
2. The client joins the camera's Wi-Fi (SSID `DIRECT-xxxx:ILCE-6000`; password is
   shown via the camera's delete/trash button).
3. In AP mode the camera is the gateway at a fixed `192.168.122.1`.
4. Service description: `GET http://192.168.122.1:64321/DmsDescPush.xml` (and the
   linked service-description XML) to find the ContentDirectory control URL.
5. Enumerate photos via a UPnP ContentDirectory `Browse` SOAP action
   (`ObjectID=0`, `BrowseDirectChildren`). Each item exposes `<res>` URLs for
   thumbnail, reduced, and original images.
6. Download images via plain HTTP GET on those `<res>` URLs.

Limitations:
- This path serves **JPEG**; RAW (.ARW) is downscaled to JPEG by the camera. RAW
  requires a USB card reader (out of scope).
- SSDP discovery can be **skipped** in AP mode because the address is fixed — hit
  the known endpoints directly for reliability. Keep SSDP as an optional fallback.

## Functional requirements
- List all photos with thumbnails.
- Multi-select in the web UI (checkbox + shift-range).
- Download selected: stream originals to the browser (desktop), or save to
  `~/storage/dcim` on Termux (`POST /api/save`).
- Show basic metadata (filename, date) from the Browse result when available.
- Graceful errors when not connected to the camera AP.

## Non-goals (v1)
- Remote shooting / camera control.
- RAW transfer (protocol limitation).
- Infrastructure-mode "Send to Computer" PTP/IP path (possible future module).
- A native Android APK (Termux is the v1 Android story).

## HTTP API
```
GET  /api/list        -> [{ id, name, date, thumbUrl, fullUrl }]
GET  /api/thumb/:id   -> image bytes (proxied)
GET  /api/photo/:id   -> original JPEG (proxied, content-disposition: attachment)
POST /api/save        -> { "ids": [...] }   # Termux: writes to ~/storage/dcim
```

## Cross-platform behavior
- Desktop: after starting, auto-open the default browser to the localhost URL.
- Termux/Android: print the URL (optionally `termux-open-url`); save target is
  `~/storage/dcim` (needs `termux-setup-storage`); call `termux-wake-lock` so the
  server survives backgrounding.
- Always bind `127.0.0.1` only.

## Distribution
- **GoReleaser + GitHub Actions** builds all targets and publishes a GitHub Release
  (the free hosting backbone). Layer convenience install fronts on top:
  - `curl install.sh` (macOS / Linux / Termux), detects OS + arch.
  - PowerShell `install.ps1` (Windows).
  - Homebrew tap (auto-published by GoReleaser).
  - Scoop / winget (Windows).
  - `go install github.com/<owner>/sony-camera-portal@latest` for anyone with Go.
- Build targets: darwin/arm64, darwin/amd64, windows/amd64, windows/arm64,
  linux/amd64, linux/arm64, android/arm64 (Termux).

## Milestones
- **M1** `camera/`: fetch service description, ContentDirectory `Browse`, return a
  typed photo list. Unit-test the XML parsing against a captured DmsDescPush.xml.
- **M2** `server/`: HTTP server, embed `web/`, `/api/list` + thumb/photo proxy routes.
- **M3** `web/`: thumbnail grid, multi-select, download flow.
- **M4** cross-platform polish: browser auto-open vs print URL, Termux save path, flags.
- **M5** release: `.goreleaser.yaml`, GitHub Actions workflow, `install.sh`, Homebrew tap.

## Open questions (verify against a live a6000)
- Exact `<res>` URL shapes and ports vary by firmware — pin parsing to the actual
  `DmsDescPush.xml` output from the camera.
- Confirm the ContentDirectory control URL and `Browse` envelope on this body.
