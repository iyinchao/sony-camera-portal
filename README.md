# sony-camera-portal

A single self-contained Go binary that runs a **local web gallery** for browsing,
selecting, and downloading JPEG photos from a Sony a6000 (and similar
PlayMemories / Imaging Edge cameras) over the **camera's own Wi-Fi access
point** — no Sony app, no cloud, works fully offline.

```
Browser → http://127.0.0.1:PORT  (same-origin, no CORS)
        → Go server              (serves the embedded React UI + /api proxy)
        → camera                 (server-side UPnP/DLNA fetch; CORS/mixed-content don't apply)
```

## Try it with a camera

1. On the camera: **Menu → Send to Smartphone → Select on Smartphone**.
2. Join the camera's Wi-Fi (SSID `DIRECT-xxxx:ILCE-6000`; the password shows when
   you press the camera's trash button).
3. Run the binary and it opens your browser to the gallery:
   ```sh
   ./sony-camera-portal --port 8080
   ```
   The camera is auto-discovered via SSDP (its AP IP varies by firmware). To pin
   it: `--camera-host 10.0.0.1`.

Select photos (click to toggle, shift-click for a range) and **Download
selected** to save the originals.

> Only JPEG is available over this path; the camera downscales RAW (.ARW). RAW
> needs a USB card reader (out of scope).

## Develop without a camera (mock mode)

`--mock N` serves N fake photos (generated placeholder JPEGs) instead of a real
camera, so you can exercise the UI — grid, multi-select, shift-range, download —
with no camera connected.

```sh
# Quick look (built binary)
./sony-camera-portal --mock 24            # auto-opens the browser

# Go dev loop (recompiles on start; serves the built web/dist)
go run . --mock 24
```

For **frontend work with hot-reload**, run the Go backend and the Vite dev
server side by side — Vite proxies `/api` to Go, so you get HMR on the React
source with mock data:

```sh
# terminal 1 — Go backend serving only the mock /api
go run . --port 8080 --mock 24 --no-open

# terminal 2 — Vite dev server with hot-reload
cd web && npm run dev                      # then open http://localhost:5173
```

```
Browser → Vite :5173 (React source, hot-reload)
                │ /api/* proxied
                ▼
            Go :8080 (--mock fake photos)
```

Edit `web/src/*` and the browser updates instantly. Bump `--mock 100` to test the
grid with many photos.

## Build from source

Requires Go 1.25+ and Node 18+ (build-time only).

```sh
# 1. Build the frontend → web/dist/ (embedded into the binary)
cd web && npm ci && npm run build && cd ..

# 2. Build the static binary
CGO_ENABLED=0 go build -o sony-camera-portal .
```

See `docs/DEVELOPMENT.md` for the full dev workflow (hot-reload, cross-compile,
tests) and `SPEC.md` for the camera protocol and milestones.

## Flags

| Flag | Default | Meaning |
|------|---------|---------|
| `--port` | `8080` | localhost port (server binds `127.0.0.1` only) |
| `--camera-host` | _(SSDP discover)_ | camera IP; fallback `192.168.122.1` |
| `--no-open` | `false` | don't auto-open the browser |

## Layout

- `main.go` — CLI, embeds `web/dist`, starts the server
- `camera/` — UPnP ContentDirectory client (SSDP discover, Browse, fetch)
- `server/` — HTTP server: embedded UI + `/api/list|thumb|photo` proxy
- `web/` — React + Vite + TypeScript frontend (`npm run build` → `web/dist/`)
- `scripts/grab.py` — offline tool to capture camera fixtures for tests
