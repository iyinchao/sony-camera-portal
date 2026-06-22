# sony-camera-portal — Claude Code project guide

## What this is
A single self-contained binary that runs a **local web gallery** for browsing,
selecting, and downloading photos from a Sony a6000 (and similar PlayMemories /
Imaging Edge cameras) over the **camera's own Wi-Fi access point** — no Sony app,
no cloud, works fully offline.

Runs on macOS, Windows, Linux, and Android (inside Termux). **One Go codebase.**

Read `SPEC.md` for full requirements, the camera protocol, and milestones.

## Architecture (one sentence)
Browser → `http://127.0.0.1:PORT` (same-origin, no CORS) → Go server → camera at
`http://192.168.122.1` (server-side fetch, so CORS / mixed-content limits don't
apply). The web UI is embedded into the binary via `go:embed`, so the user just
runs the binary and opens the printed localhost URL.

## Tech & conventions
- Go backend. Module path: `github.com/<owner>/sony-camera-portal`. Standard
  library first; `net/http` for the server, `encoding/xml` for UPnP SOAP.
- Frontend: **React + Vite + TypeScript**, built to static assets in `web/dist/`
  and embedded via `go:embed`. The npm build is a BUILD-TIME step only — the
  bundle ships locally (no CDN, no runtime internet), preserving offline use.
- Build static binaries with `CGO_ENABLED=0`.
- `gofmt` and `go vet` must be clean. Table-driven tests in `_test.go`.
- The HTTP server binds `127.0.0.1` **only** (never `0.0.0.0`).
- No telemetry. No network calls at runtime except to the camera.

## Layout
- `main.go`   CLI: flags, start server, open browser (desktop) / print URL (Termux)
- `server/`   HTTP: serves the embedded web UI (`web/dist`) + `/api` proxy routes
- `camera/`   camera client: AP endpoints, UPnP ContentDirectory Browse, list model
- `web/`      React + Vite + TS frontend (source); `npm run build` → `web/dist/`

## API routes
- `GET  /api/list`      → JSON `[{ id, name, date, thumbUrl, fullUrl }]`
- `GET  /api/thumb/:id` → proxied thumbnail bytes
- `GET  /api/photo/:id` → proxied original JPEG (download)
- `POST /api/save`      → (Termux) save selected ids to `~/storage/dcim`

## Build / run
- Frontend build: `cd web && npm ci && npm run build` → emits `web/dist/`
  (required before `go build`/`go run`, since `server/` embeds `web/dist`)
- Dev run:      `go run . --port 8080`  (frontend hot-reload: `cd web && npm run dev`, Vite proxies `/api` to Go)
- Build local:  `CGO_ENABLED=0 go build -o sony-camera-portal .`
- Termux build: `GOOS=android GOARCH=arm64 CGO_ENABLED=0 go build`
- Other targets: darwin/arm64, darwin/amd64, windows/amd64, linux/amd64, linux/arm64
- Release:      GoReleaser (`.goreleaser.yaml`) → GitHub Releases + Homebrew tap

## Hard constraints (do not regress)
- The "Send to Smartphone / Select on Smartphone" path returns **JPEG only**; the
  camera downscales RAW (.ARW) over this path. Never promise RAW here.
- In AP mode the camera is fixed at `192.168.122.1`; keep it configurable but
  default to that.
- Stay offline-capable: no CDN, no internet dependency at runtime.
