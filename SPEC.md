# sony-camera-portal — Project spec

## Goal
Browse, select, and download photos from a Sony a6000 over the **camera's Wi-Fi
access point**, using only a **local web UI** in the user's browser. No Sony app,
no cloud, works offline (the camera AP has no internet).

Runs on macOS / Windows / Linux desktops, on Android (Termux), and on **iOS
(inside iSH)**. **One Rust codebase**, distributed as a single self-contained
binary that cross-compiles to fully-static targets (incl. `i686-unknown-linux-
musl` for iSH).

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
Confirmed against a real ILCE-6000. Values vary by firmware — discover, don't
hard-code.
1. On the camera: Menu → Send to Smartphone → **Select on Smartphone**. The camera
   becomes a Wi-Fi AP and runs a UPnP/DLNA stack.
2. The client joins the camera's Wi-Fi (SSID `DIRECT-xxxx:ILCE-6000`; password is
   shown via the camera's delete/trash button).
3. In AP mode the camera is the gateway. The IP **varies by firmware** — this
   body uses `10.0.0.1`, NOT the commonly-documented `192.168.122.1`.
4. Service description: `GET http://<host>:64321/DmsDesc.xml` → parse the
   ContentDirectory `controlURL` (relative; resolve against the description URL).
5. Enumerate photos via a UPnP ContentDirectory `Browse` SOAP action
   (`ObjectID=0`, `BrowseDirectChildren`), **recursing containers** (root →
   PhotoRoot → date containers → image items).
6. Each item has `dc:title` (filename), `dc:date`, and four `<res>` URLs
   distinguished by `DLNA.ORG_PN` in protocolInfo: `JPEG_TN` (thumbnail), `_SM`,
   `_LRG`, and a PN-less entry (the full-resolution original). Media is served
   from a separate port (e.g. `:60151`).
7. Download images via plain HTTP GET on those `<res>` URLs.

Discovery:
- **SSDP M-SEARCH** finds the camera on desktop. On **iOS/iSH it is blocked**
  (no multicast entitlement; send fails EHOSTUNREACH).
- Fallback (works on iSH): derive candidate gateways from the local IP
  (`getsockname` trick) and probe their `DmsDesc.xml`, accepting only a Sony
  device. The user can always type the IP in the web UI.

Limitations:
- This path serves **JPEG**; RAW (.ARW) is downscaled to JPEG by the camera. RAW
  requires a USB card reader (out of scope).

## Functional requirements
- The server starts **without** a camera; connecting is driven by the web UI.
- Connect / type an IP / auto-discover / reconnect / switch cameras at runtime.
- List all photos with thumbnails, grouped by date.
- Multi-select in the web UI (checkbox + shift-range + per-day select).
- Download selected: stream originals to the browser.
- Show basic metadata (filename, date) from the Browse result.
- Graceful, actionable errors when not connected (a connect panel, not a crash).

## Non-goals (v1)
- Remote shooting / camera control.
- RAW transfer (protocol limitation).
- Infrastructure-mode "Send to Computer" PTP/IP path (possible future module).
- Termux `POST /api/save` to `~/storage/dcim` (possible future Android nicety).

## HTTP API
```
GET  /api/state       -> { connected, host, error, photoCount }
POST /api/connect     -> body { host? }   # set/auto-discover + validate the camera
GET  /api/list        -> [{ id, name, date, thumbUrl, fullUrl }]  (503 if not connected)
GET  /api/thumb/:id   -> image bytes (proxied)
GET  /api/photo/:id   -> original JPEG (proxied, content-disposition: attachment)
```

## Cross-platform behavior
- Desktop: after starting, auto-open the default browser to the localhost URL.
- Android (Termux) / iOS (iSH): the printed URL is the entry point; keep the host
  app alive (Termux wake-lock / iSH Location Services or iPad Split View).
- Always bind `127.0.0.1` only.

## Distribution
- **GitHub Actions** cross-compiles all targets with `cargo` (+ `cargo-zigbuild`
  for musl) and publishes a GitHub Release. Convenience fronts on top:
  - `curl install.sh` (macOS / Linux / Termux), detects OS + arch.
  - PowerShell `install.ps1` (Windows); Homebrew tap; Scoop / winget.
  - `cargo install` for anyone with a Rust toolchain.
- Build targets: darwin/arm64+amd64, windows/amd64+arm64, linux/amd64+arm64,
  android/arm64 (Termux), `i686-unknown-linux-musl` (iSH/iOS).

## Status
The Rust product (`packages/`) implements the camera client, the localhost
server with runtime connection management, and the React UI; it cross-compiles to
the iSH target. See `openspec/changes/` for the spec-driven history. Remaining:
release tooling (CI + install fronts) and the optional Termux save path.

## Resolved / open questions
- `<res>` URL shapes and ports: **resolved** — pinned to captured fixtures
  (`packages/camera/testdata/`), selected by `DLNA.ORG_PN`.
- ContentDirectory control URL + `Browse` envelope: **resolved** against this
  body; tests cover the parsing.
- Open: behavior across other firmwares/bodies (host/ports may differ again).
