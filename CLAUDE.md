# sony-camera-portal — Claude Code project guide

## What this is
A single self-contained binary that runs a **local web gallery** for browsing,
selecting, and downloading photos from a Sony a6000 (and similar PlayMemories /
Imaging Edge cameras) over the **camera's own Wi-Fi access point** — no Sony app,
no cloud, works fully offline.

Runs on macOS, Windows, Linux, Android (Termux), and **iOS (inside iSH)**.
**One Rust codebase** (the canonical impl). The old Go MVP and the Rust PoC are
archived under `docs/prototype/` for reference only.

Read `SPEC.md` for the camera protocol; `openspec/` for the spec-driven plan.

## Architecture (one sentence)
Browser → `http://127.0.0.1:PORT` (same-origin, no CORS) → local Rust server →
camera (server-side fetch, so CORS / mixed-content limits don't apply). The web
UI is embedded into the binary via `rust-embed`, so the user just runs the binary
and opens the printed localhost URL.

## Tech & conventions
- **Rust**, blocking I/O (no async runtime — best fit for iSH's emulated env).
  A hand-rolled minimal HTTP client over `TcpStream`; the only socket option is a
  bounded **`connect_timeout`** (3s, poll-based — NOT the `setsockopt` timeouts
  iSH rejects with EINVAL) so an unreachable host fails fast instead of hanging.
  `tiny_http` server run over a **small fixed worker pool** (concurrent, so a slow
  `/api/connect` never freezes `/api/state`/media/assets), `roxmltree` for UPnP
  SOAP, `rust-embed` for the web bundle, `serde_json`. Pure-Rust deps only, so
  `i686-unknown-linux-musl` (iSH) cross-compiles cleanly.
- Frontend: **React + Vite + TypeScript**, built to `packages/web/dist` and
  embedded. The npm build is BUILD-TIME only; the bundle ships locally (no CDN,
  no runtime internet), preserving offline use. UI uses **Tailwind v4 + Radix**
  (system sans-serif, auto light/dark via `prefers-color-scheme`). The gallery is
  **virtualized** (`@tanstack/react-virtual`, a flat header+tile-row model →
  constant DOM) with infinite scroll, date grouping + sort/group toggles, and a
  controlled `react-photo-view` `PhotoSlider` lightbox over the full loaded list.
- `cargo fmt` + `cargo clippy --all-targets` + `cargo test` must be clean.
- The HTTP server binds `127.0.0.1` **only** (never `0.0.0.0`). No telemetry.

## Layout (Cargo workspace)
- `packages/camera/`  lib: DLNA client — discover (SSDP + gateway probe), Browse, fetch
- `packages/server/`  lib: HTTP server, `/api` proxy, runtime connection state (`AppState`)
- `packages/cli/`     bin `sony-camera-portal`: flags, `rust-embed` web bundle, start
- `packages/web/`     React + Vite + TS frontend; `npm run build` → `packages/web/dist/`
- `docs/prototype/`   archived Go MVP + Rust PoC (reference, not built)

## API routes
- `GET  /api/state`     → `{ connected, host, error, photoCount }`
- `POST /api/connect`   → body `{ host? }`; sets/auto-discovers + validates the camera
- `GET  /api/list?offset&limit` → `{ photos: [{ id, name, date, thumbUrl, fullUrl }], total, hasMore }` (paged; 503 if not connected)
- `GET  /api/thumb/:id` → proxied thumbnail bytes (`Cache-Control: immutable`)
- `GET  /api/photo/:id` → proxied original JPEG (Content-Disposition: attachment; `Cache-Control: immutable`)

## Build / run
- Frontend build: `cd packages/web && npm ci && npm run build` → `packages/web/dist/`
  (needed before `cargo build`; debug builds read it from disk at runtime)
- Dev run:        `cargo run -- --mock 18` (no camera) — then connect from the web UI
  - Frontend hot-reload: `cd packages/web && npm run dev` (Vite proxies `/api` → :8080)
- Build:          `cargo build --release`
- iSH (iOS):      `cargo zigbuild --release --target i686-unknown-linux-musl` (static 32-bit ELF)
- Other targets:  darwin/arm64+amd64, windows, linux, android (Termux) via cargo cross

## Connection model (do not regress)
- The server starts **without** a camera and never connects on its own. All
  connecting is driven by the web UI via `/api/connect` (auto-discover, or a
  user-typed IP); the user can change IP / reconnect / switch cameras anytime.
  `/api/connect` validates before swapping, so a bad IP never drops a good
  connection. (There is intentionally no `--camera-host` flag.)
- Connecting is **concurrent, bounded, and supersedable**: the slow discovery
  holds no lock (other requests are served meanwhile), each candidate connect is
  bounded (3s `connect_timeout`), and an `epoch` makes a newer connect win — a
  stale attempt's result is discarded, never clobbering newer state. An
  already-connected camera is reused on reload (the UI bootstraps from
  `/api/state` and only connects when not already connected).

## Hard constraints (do not regress)
- The "Send to Smartphone / Select on Smartphone" path returns **JPEG only**; the
  camera downscales RAW (.ARW) over this path. Never promise RAW here.
- Camera host varies by firmware (this body is `10.0.0.1`, not the SPEC's
  documented `192.168.122.1`); discover, don't hard-code.
- Discovery: SSDP multicast works on desktop but is **blocked on iOS/iSH**; fall
  back to local-IP gateway probing (the camera is the AP gateway).
- Stay offline-capable: no CDN, no internet dependency at runtime.
