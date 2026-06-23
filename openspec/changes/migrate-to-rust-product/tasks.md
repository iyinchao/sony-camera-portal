## 1. Archive prototypes

- [x] 1.1 `git mv` Go sources (`camera/`, `server/`, `main.go`, `mock.go`, `go.mod`) → `docs/prototype/go/`; `docs/prototype/README.md` notes both prototypes are retired references
- [x] 1.2 `git mv rust-poc/` → `docs/prototype/rust-poc/`
- [x] 1.3 Update `.gitignore` for the new paths (workspace `/target/`, `packages/web/{dist,node_modules}`, prototype build dirs)

## 2. Workspace skeleton (compiles green)

- [x] 2.1 Root `Cargo.toml` workspace with members `packages/camera`, `packages/server`, `packages/cli`
- [x] 2.2 Empty `packages/{camera,server}` lib crates + `packages/cli` bin crate that builds: `cargo build` succeeds

## 3. camera lib (port + verify offline)

- [ ] 3.1 Move fixtures to `packages/camera/testdata/` (`DmsDesc.xml`, `browse_response.xml`)
- [ ] 3.2 Port model + iSH-safe blocking HTTP client (no socket options) into `packages/camera`
- [ ] 3.3 Port discovery: SSDP → local-IP gateway candidates (getsockname) → known-host fallback
- [ ] 3.4 Port device-description parse + recursive Browse + `<res>` selection; port unit tests (`cargo test -p camera` green against fixtures)

## 4. server lib (proxy + connection state)

- [ ] 4.1 `AppState` with `Mutex<Option<Target>>` + cached `id→photo` map
- [ ] 4.2 `GET /api/state`, `POST /api/connect` (validate-then-swap), returning JSON state
- [ ] 4.3 `GET /api/list` against current target; structured not-connected response when none
- [ ] 4.4 `GET /api/thumb/{id}` / `GET /api/photo/{id}` proxy (Content-Disposition on photo); 404 unknown id
- [ ] 4.5 Tests with a stub camera: list maps to proxied URLs; connect validates; disconnected list is non-2xx (`cargo test -p server`)

## 5. cli bin (embed + serve)

- [ ] 5.1 `packages/cli`: flags `--port`, `--camera-host` (optional hint), `--no-open`, `--mock N`
- [ ] 5.2 Embed `packages/web/dist` via `rust-embed`; serve at `/`; bind `127.0.0.1`; print URL; start without requiring a camera

## 6. web frontend (move + connection UI)

- [ ] 6.1 `git mv web/` → `packages/web/`; fix `vite.config.ts` outDir and the cli embed path
- [ ] 6.2 Add a connection state machine + connect panel (auto-retry + manual IP input) shown when not connected
- [ ] 6.3 Toolbar status (active host) + "change camera" control that POSTs `/api/connect` and refreshes
- [ ] 6.4 `npm run build` emits `packages/web/dist`; gallery still grids/selects/downloads once connected

## 7. Docs + verify

- [ ] 7.1 Update `CLAUDE.md`, `SPEC.md`, `docs/DEVELOPMENT.md`, `README.md` for Rust + new layout + connection model
- [ ] 7.2 `cargo fmt --check`, `cargo clippy`, `cargo test` all clean
- [ ] 7.3 Cross-build `i686-unknown-linux-musl` (iSH) succeeds; binary is static 32-bit ELF
- [ ] 7.4 Run with `--mock`, open the UI, verify connect panel + change-camera + grid/select/download (screenshot)
