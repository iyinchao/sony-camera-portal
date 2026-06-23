# sony-camera-portal

A single self-contained **Rust** binary that runs a **local web gallery** for
browsing, selecting, and downloading JPEG photos from a Sony a6000 (and similar
PlayMemories / Imaging Edge cameras) over the **camera's own Wi-Fi access
point** — no Sony app, no cloud, works fully offline.

```
Browser → http://127.0.0.1:PORT  (same-origin, no CORS)
        → local Rust server      (serves the embedded React UI + /api proxy)
        → camera                 (server-side UPnP/DLNA fetch; CORS/mixed-content don't apply)
```

Runs on macOS, Windows, Linux, Android (Termux), and **iOS (inside iSH)** — the
binary cross-compiles to a ~840 KB fully-static `i686-unknown-linux-musl` for iSH.

## Try it with a camera

1. On the camera: **Menu → Send to Smartphone → Select on Smartphone**.
2. Join the camera's Wi-Fi (SSID `DIRECT-xxxx:ILCE-6000`; the password shows when
   you press the camera's trash button).
3. Run the binary; it opens your browser to the gallery:
   ```sh
   ./sony-camera-portal --port 8080
   ```
4. The UI tries to **auto-discover** the camera. If that fails (e.g. on iSH, where
   iOS blocks multicast), type the camera IP (e.g. `10.0.0.1`) in the connect
   panel. You can change the IP / reconnect from the UI anytime.

Select photos (click to toggle, shift-click for a range) and **Download
selected** to save the originals.

> Only JPEG is available over this path; the camera downscales RAW (.ARW). RAW
> needs a USB card reader (out of scope).

## Develop without a camera (mock mode)

`--mock N` serves N fake photos (SVG placeholders) so you can exercise the UI —
grid, date-grouping, multi-select, shift-range, download — with no camera.

```sh
cargo run -- --mock 24            # auto-opens the browser
```

For **frontend work with hot-reload**, run the Rust backend and the Vite dev
server side by side — Vite proxies `/api` to the backend, so you get HMR on the
React source with mock data. One command starts both:

```sh
./scripts/dev.sh          # backend (mock) + Vite HMR; open http://localhost:5173
./scripts/dev.sh 100      # 100 mock photos
./scripts/dev-stop.sh     # stop both
```

Or run them in two terminals:

```sh
# terminal 1 — Rust backend serving the mock /api
cargo run -- --port 8080 --mock 24 --no-open

# terminal 2 — Vite dev server with hot-reload
cd packages/web && npm run dev             # then open http://localhost:5173
```

Edit `packages/web/src/*` and the browser updates instantly.

## Build from source

Requires a Rust toolchain and Node 18+ (frontend, build-time only).

```sh
# 1. Build the frontend → packages/web/dist/ (embedded into the binary)
cd packages/web && npm ci && npm run build && cd ../..

# 2. Build the binary
cargo build --release             # target/release/sony-camera-portal

# iOS / iSH: a fully-static 32-bit musl binary
#   cargo install cargo-zigbuild && brew install zig && rustup target add i686-unknown-linux-musl
cargo zigbuild --release --target i686-unknown-linux-musl
```

See `docs/DEVELOPMENT.md` for the full dev workflow and `SPEC.md` for the camera
protocol. The Go MVP and the original Rust PoC are archived under
`docs/prototype/`.

## Flags

| Flag | Default | Meaning |
|------|---------|---------|
| `--port` | `8080` | localhost port (server binds `127.0.0.1` only) |
| `--mock N` | _(off)_ | serve N synthetic photos instead of a camera |
| `--no-open` | `false` | don't auto-open the browser |

The camera is connected from the web UI (auto-discover or a typed IP), not a CLI
flag — so the server starts instantly and never blocks on an unreachable camera.

## Layout (Cargo workspace)

- `packages/camera/` — DLNA client: discover (SSDP + gateway probe), Browse, fetch
- `packages/server/` — HTTP server: embedded UI + `/api` proxy + runtime connection state
- `packages/cli/` — the `sony-camera-portal` binary (flags, embeds the web bundle)
- `packages/web/` — React + Vite + TypeScript frontend
- `scripts/grab.py` — offline tool to capture camera fixtures for tests
- `docs/prototype/` — archived Go MVP + Rust PoC (reference)
