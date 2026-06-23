## Why

The Rust PoC is confirmed working end-to-end on a real iSH/iOS install (serves
the gallery, shows real camera photos), while the Go implementation has never run
on iSH and is structurally risky there (its `net/http` netpoller relies on epoll,
the weakest part of iSH's emulation). Rust also cross-compiles to tiny fully
static binaries on every target. We are therefore making **Rust canonical** and
retiring Go to a reference prototype.

At the same time the current design couples server startup to camera
connectivity: if the camera isn't reachable at launch, the tool is dead. We want
the server to start unconditionally and let the user **connect, type an IP, or
reconnect/switch cameras from the web UI** — which also sidesteps the fact that
auto-discovery is impossible on iSH (iOS blocks multicast).

## What Changes

- **Archive prototypes** to `docs/prototype/`: the Go code (`camera/`, `server/`,
  `main.go`, `mock.go`, `go.mod`, …) → `docs/prototype/go/`; the current
  `rust-poc/` → `docs/prototype/rust-poc/`. Reference only, not built.
- **Restructure into a Cargo workspace** with no loose source at the repo root:
  `packages/camera` (lib), `packages/server` (lib), `packages/cli` (bin, embeds
  the web bundle), `packages/web` (the React frontend, moved from `web/`).
- **Port the validated Rust PoC** into this structure as the real product:
  iSH-safe blocking HTTP client, SSDP-with-fallback discovery, recursive Browse,
  proxy routes, and serving the **React** UI via `rust-embed` (replacing the
  PoC's inline HTML — the React frontend is reused unchanged).
- **Add runtime connection management** (new `connection-manager` capability):
  the server starts without a camera; `GET /api/state` reports connection status;
  `POST /api/connect` sets/auto-discovers/validates a camera host; the web UI
  shows a connect panel (auto + manual IP) and a toolbar control to change IP /
  reconnect mid-session.
- Update `CLAUDE.md`, `SPEC.md`, `docs/DEVELOPMENT.md`, `README.md` for the Rust
  product, the new layout, and the connection model.

## Capabilities

### New Capabilities
- `connection-manager`: runtime camera connection state — the server runs without
  a camera, and the user connects / supplies an IP / reconnects from the web UI.

### Modified Capabilities
<!-- camera-client, gallery-server, and web-gallery are reimplemented in Rust but
     their observable behavior (enumerate, proxy, grid/select/download) is
     preserved; the reimplementation is captured in design.md + tasks.md, not as
     new requirements. The only behavioral change is the connection model, which
     the new connection-manager capability covers. -->

## Impact

- Code moves: Go → `docs/prototype/go/`; `rust-poc/` → `docs/prototype/rust-poc/`;
  `web/` → `packages/web/`. New: `Cargo.toml` (workspace), `packages/{camera,
  server,cli}`.
- Build: `go.mod`/Go toolchain no longer required to build the product; release
  pipeline (M5) retargets to `cargo` + cross-compile (incl. i686-musl for iSH).
- Dependencies (Rust, pure-Rust to keep musl cross clean): `tiny_http`,
  `roxmltree`, `serde`/`serde_json`, `rust-embed`. No async runtime, no HTTP
  client crate (hand-rolled, iSH-safe).
- Camera host is no longer a hard launch requirement; `--camera-host` becomes an
  optional initial hint, overridable at runtime via `/api/connect`.
