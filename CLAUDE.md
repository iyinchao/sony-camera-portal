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
- Dev run:        `cargo run -- --mock 18` (N synthetic photos); add
  `--mock-dir <path>` to source the images from a directory (cycling if N exceeds
  the file count; `--mock-dir` alone = one per file); `--mock-delay <s>` simulates
  the connect/discovery UX — then connect from the web UI. (Mock starts cameraless
  and connects via the UI, just like a real camera.)
  - Frontend hot-reload: `cd packages/web && npm run dev` (Vite proxies `/api` → :8080)
- **`mock` Cargo feature** (default ON for dev & tests) gates all mock code.
  **Release/distribution builds strip it with `--no-default-features`** — the
  shipped binary has no mock code and ignores `--mock`.
- Release builds (mock stripped) go through **`scripts/build.sh <platform>`**, which
  builds the web bundle then the embedded binary and collects it into **`dist/`**
  under a per-platform name (e.g. `dist/sony-camera-portal-android-arm64`):
  - `scripts/build.sh ish`        → iSH/iOS       `i686-unknown-linux-musl`   (zigbuild, static)
  - `scripts/build.sh android`    → Termux arm64  `aarch64-linux-android`     (NDK clang, PIE)
  - `scripts/build.sh linux`      → Linux x86_64  `x86_64-unknown-linux-musl` (zigbuild, static)
  - `scripts/build.sh macos`      → macOS (host)  native cargo
  - also `android32` (armv7), `linux-arm` (aarch64), `windows` (x86_64-pc-windows-gnu);
    `all` = ish + android + linux + macos.
  - **iSH / Linux** use static **musl** (zigbuild) — no libc/NDK; the kernel runs them
    directly. **Android/Termux MUST be PIE** (non-PIE static is rejected with
    "unexpected e_type: 2"), so it uses the **Android NDK** clang linker → a PIE Bionic
    binary; set `ANDROID_NDK_HOME` or install the NDK under `$ANDROID_HOME/ndk/*`
    (`NDK_API` sets the min API, default 24). The script passes `--no-default-features`.
  - Plain debug build for the host: `cargo build` (mock on); `cargo run -- --mock 18`.

## Releasing & commits
- **Conventional Commits are required.** A `cog` (cocogitto) `commit-msg` hook
  (`cog.toml`) rejects non-conforming messages. After cloning, install it once:
  `cargo install cocogitto && cog install-hook --all` (hooks live in `.git/hooks`,
  not distributed). Types: `feat` / `fix` (bump) + `build` / `ci` / `docs` /
  `chore` / `refactor` / `perf` / `test` / `style`.
- **Releases are cut with `cog bump --auto`** (`cog.toml`) — do NOT tag by hand.
  It computes the next version from the commits since the last tag, runs
  `cargo set-version` to bump the single `[workspace.package]` version (+ Cargo.lock;
  the libs inherit it), regenerates `CHANGELOG.md`, makes a `chore(version): vX.Y.Z`
  commit, and tags `vX.Y.Z`. Then `git push && git push --tags`: the tag triggers
  `.github/workflows/release.yml`, which cross-compiles via `scripts/build.sh` and
  publishes a GitHub Release with the binaries attached. Needs `cargo-edit`
  (`cargo install cargo-edit`) for `cargo set-version`.
  - We do NOT use release-plz: it runs `cargo package` on the previous tag to diff,
    which fails on this never-published workspace (inter-crate `path` deps without
    versions). cog is git/commit-only, so none of that applies.

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
