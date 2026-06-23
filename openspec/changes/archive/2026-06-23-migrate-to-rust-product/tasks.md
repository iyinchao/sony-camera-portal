## 1. Archive prototypes

- [x] 1.1 `git mv` Go sources (`camera/`, `server/`, `main.go`, `mock.go`, `go.mod`) → `docs/prototype/go/`; `docs/prototype/README.md` notes both prototypes are retired references
- [x] 1.2 `git mv rust-poc/` → `docs/prototype/rust-poc/`
- [x] 1.3 Update `.gitignore` for the new paths (workspace `/target/`, `packages/web/{dist,node_modules}`, prototype build dirs)

## 2. Workspace skeleton (compiles green)

- [x] 2.1 Root `Cargo.toml` workspace with members `packages/camera`, `packages/server`, `packages/cli`
- [x] 2.2 Empty `packages/{camera,server}` lib crates + `packages/cli` bin crate that builds: `cargo build` succeeds

## 3. camera lib (port + verify offline)

- [x] 3.1 Move fixtures to `packages/camera/testdata/` (`DmsDesc.xml`, `browse_response.xml`)
- [x] 3.2 Port model + iSH-safe blocking HTTP client (no socket options) into `packages/camera` (`http.rs`)
- [x] 3.3 Port discovery: SSDP → local-IP gateway candidates (getsockname) → known-host fallback (`discover.rs`)
- [x] 3.4 Port device-description parse + recursive Browse + `<res>` selection; public `Camera{connect,discover,list,fetch}` API; 9 unit tests green, zero warnings

## 4. server lib (proxy + connection state)

- [x] 4.1 `AppState` (`state.rs`): `Mutex<Inner{source,photos,last_error}>`; `Source` trait with `RealCamera` + `MockSource` (`source.rs`)
- [x] 4.2 `GET /api/state`, `POST /api/connect` (validate-then-swap: bad host keeps the current connection), returning JSON state
- [x] 4.3 `GET /api/list` against current source; 503 + JSON error when not connected
- [x] 4.4 `GET /api/thumb/{id}` / `GET /api/photo/{id}` proxy (Content-Disposition on photo); 404 unknown id
- [x] 4.5 Pure `handle()` router + 7 unit tests (mock source); workspace builds zero-warning, `cargo test` green

## 5. cli bin (embed + serve)

- [x] 5.1 `packages/cli`: flags `--port`, `--no-open`, `--mock N` (no camera-host flag — connecting is web-driven); browser auto-open
- [x] 5.2 Embed `packages/web/dist` via `rust-embed` (`AssetSource`); serve at `/`; bind `127.0.0.1`; start without a camera (smoke-tested: index + assets + /api/state|list|thumb)

## 6. web frontend (move + connection UI)

- [x] 6.1 `git mv web/` → `packages/web/`; vite outDir resolves to `packages/web/dist`, cli embeds `../web/dist`
- [x] 6.2 Connection state machine (`App.tsx`) + `ConnectPanel` (auto-discover + manual IP) shown when not connected
- [x] 6.3 Toolbar host chip (active host) + "change" control reopening the connect panel; `connectCamera()`/`getState()` API
- [x] 6.4 `npm run build` clean (tsc); `Gallery` keeps date-grouping/multi-select/download once connected (mock screenshots verified)

## 7. Docs + verify

- [x] 7.1 Updated `CLAUDE.md`, `SPEC.md`, `docs/DEVELOPMENT.md`, `README.md` for Rust + new layout + web-driven connection (no `--camera-host`)
- [x] 7.2 `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` all clean (5 test binaries)
- [x] 7.3 Cross-build `i686-unknown-linux-musl` (iSH) succeeds: static 32-bit ELF, ~840 KB
- [x] 7.4 `--mock` run verified in-browser: gallery + host chip + connect panel (manual IP / auto-discover / cancel) — screenshots
