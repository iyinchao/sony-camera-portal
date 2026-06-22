## 1. Project scaffold

- [x] 1.1 Confirm `go.mod` (`github.com/iyinchao/sony-camera-portal`, Go 1.26) and create package dirs `camera/`, `server/`, `web/`
- [x] 1.2 Add `main.go` with `--port` (8080), `--camera-host` (SSDP-discover default), `--no-open` flags; embeds `web/dist`, binds 127.0.0.1, auto-opens browser; `go build` succeeds (9.6MB binary)

## 2. camera/ — client (M1)

- [x] 2.1 Capture real fixtures into `camera/testdata/`: `DmsDesc.xml` and a `Browse` DIDL-Lite response (via `scripts/grab.py`; camera was at 10.0.0.1, not 192.168.122.1)
- [x] 2.2 Define typed model `Photo{ ID, Name, Date, ThumbURL, FullURL }` and a `Client{ Host, DescURL }` struct
- [x] 2.3 Implement device-description parse → ContentDirectory absolute `controlURL` + `serviceType`; table-driven test against the fixture
- [x] 2.4 Implement `Browse` SOAP request builder + DIDL-Lite response parser → `[]Photo`; select thumb (JPEG_TN) vs original (PN-less) `<res>` by DLNA.ORG_PN; table-driven test against the fixture
- [x] 2.5 Implement paging: `browseContainer` loops `StartingIndex` until `NumberReturned`/`TotalMatches` exhausted, recursing into child containers from root
- [x] 2.6 Implement `Open(url)` streaming reader + content type via HTTP GET (server holds the id→url map and proxies; replaces per-id FetchThumb/FetchOriginal)
- [x] 2.7 Map connection failures to a friendly `ErrNotConnected`; SSDP `Discover` so the host isn't hard-coded; `gofmt`/`go vet`/`go test` clean

## 3. server/ — HTTP + proxy (M2)

- [x] 3.1 `main.go` binds `127.0.0.1` only via `net.Listen` and prints the localhost URL on start (smoke-tested)
- [x] 3.2 Embed `web/dist` via `go:embed` (in main) and serve it at `/` with `http.FileServerFS`; httptest covers it
- [x] 3.3 `GET /api/list`: calls the camera, builds an `id → photo` map, returns JSON with proxied `thumbUrl=/api/thumb/:id`, `fullUrl=/api/photo/:id`; httptest with a stub camera
- [x] 3.4 `GET /api/thumb/{id}`: streams proxied thumbnail bytes with the camera's content type; 404 on unknown id
- [x] 3.5 `GET /api/photo/{id}`: streams original with `Content-Disposition: attachment; filename=...`; 404 on unknown id
- [x] 3.6 Error path: unreachable camera → `/api/list` returns 503 + JSON `{"error":...}` telling the user to join the camera Wi-Fi (httptest + live smoke test)

## 4. web/ — gallery UI (M3, React + Vite + TS)

- [x] 4.0 Scaffold `web/` (Vite React-TS): `package.json`, `vite.config.ts` (base `./`, build.outDir `dist`, dev proxy `/api`→`:8080`), `tsconfig`, `index.html`, `src/main.tsx`; `npm run build` emits `web/dist/` (verified: 47KB gzip JS, 0 prod-dep vulns)
- [x] 4.1 `App.tsx` fetches `/api/list` and renders a thumbnail grid (code complete; e2e pending server)
- [x] 4.2 Empty/error state component when `/api/list` fails or is empty
- [x] 4.3 Per-tile checkbox multi-select with a live selected-count (React state)
- [x] 4.4 Shift-click range selection between two tiles (inclusive)
- [x] 4.5 "Download selected" triggers per-photo download; no-op/disabled when nothing selected

## 5. End-to-end verification

- [x] 5.1 `gofmt -l . && go vet ./... && go test ./...` all clean
- [ ] 5.2 On the camera Wi-Fi: run the binary, open `http://127.0.0.1:8080`, verify grid loads with real photos, multi-select + download work (requires live camera AP — user-run)
- [x] 5.3 Add `README.md` with run/build instructions; JPEG-only enforced in `camera/` (no `.ARW` advertised, tested) and server binds loopback only (`net.Listen 127.0.0.1`)
