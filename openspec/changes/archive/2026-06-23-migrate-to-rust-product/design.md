## Context

Two prototypes exist: a polished Go MVP (camera + server + React, ~600 LoC,
tests) and a Rust PoC (`rust-poc/`) that is confirmed working on iSH/iOS. We are
consolidating on Rust and reorganizing a root that has accumulated loose source
plus scripts/config. We also change the connection model from
"connect-at-startup-or-die" to runtime-managed.

## Goals / Non-Goals

**Goals:**
- One canonical Rust codebase, cleanly organized as a Cargo workspace.
- Reuse the React frontend unchanged (embedded via rust-embed).
- Server starts without a camera; connect / change-IP / reconnect from the UI.
- Preserve all current behavior (enumerate, grid, multi-select, download) and
  iSH compatibility (blocking I/O, no socket options, probe-based discovery).
- Prototypes preserved under `docs/prototype/` for reference.

**Non-Goals:**
- No new gallery features (preview/lightbox, virtualization beyond what exists).
- No release tooling rework yet (GoReleaser→cargo retarget is a later change).
- Not deleting the Go code — it is archived, not removed.

## Decisions

- **Cargo workspace under `packages/`.** Members: `camera` (lib), `server` (lib),
  `cli` (bin). `web/` moves to `packages/web`. Rationale: keeps the root clean
  (only `Cargo.toml`, `Cargo.lock`, scripts/, openspec/, docs/, top-level md),
  groups the three concerns the user named, and makes camera/server reusable
  libs. Alternative (single crate, `src/` modules) is simpler but doesn't group
  the frontend or give lib boundaries; rejected for this size.
- **Reuse React via `rust-embed`.** `packages/cli` embeds `../web/dist` with
  `#[derive(RustEmbed)]` and serves it through `tiny_http`. The PoC's inline HTML
  is dropped. `web/dist` keeps a committed placeholder so `cargo build` works
  before `npm run build`.
- **Runtime connection state in `packages/server`.** An `AppState` holds
  `Mutex<Option<Target>>` where `Target = { host, control_url, service_type }`
  and a cached `id → photo` map. Routes operate against the current target;
  no target ⇒ a structured "not connected" response, not a crash.
- **Discovery stays layered and iSH-safe** (ported from the PoC): SSDP multicast
  (desktop) → local-IP-derived gateway candidates (getsockname) → known-host
  fallback, all behind `/api/connect`; a user-typed IP is the manual override.
  All HTTP uses the hand-rolled blocking client with no socket options.
- **`/api/connect` validates before committing.** It fetches+parses DmsDesc for
  the candidate host; only on success does it replace the live target, so a bad
  IP doesn't drop an existing good connection.
- **Frontend connection state machine.** `disconnected | connecting | connected |
  error`; a connect panel (auto-retry + manual IP) when not connected, and a
  toolbar status + "change camera" control to reconnect anytime. The existing
  grid/select/download is unchanged once connected.

## Risks / Trade-offs

- [Reviewers lose the Go history if archived sloppily] → Use `git mv` so history
  follows the files into `docs/prototype/go/`.
- [`rust-embed` in dev needs `web/dist` to exist] → committed placeholder + the
  documented "npm build before cargo build" order; optional dev mode could serve
  from disk later.
- [Big-bang restructure breaks the build mid-flight] → Tasks are ordered so each
  phase compiles: scaffold workspace empty-green, port camera (tests pass), port
  server, wire cli+embed, then frontend connection UI.
- [`/api/connect` while a request is in flight] → state behind a Mutex; the
  validate-then-swap keeps readers consistent.

## Migration Plan

1. `git mv` Go files → `docs/prototype/go/`; `rust-poc/` → `docs/prototype/rust-poc/`.
2. Create workspace `Cargo.toml` + `packages/{camera,server,cli}` skeletons that
   compile (empty-green).
3. Port camera lib (+ move `testdata/` fixtures, port the Rust tests).
4. Port server lib: routes/proxy + the new `AppState` connection model + tests.
5. `packages/cli`: flags, rust-embed `../web/dist`, bind 127.0.0.1, start.
6. Move `web/` → `packages/web`; add the connection UI; `npm run build`.
7. Update docs (CLAUDE/SPEC/DEVELOPMENT/README); update `.gitignore` paths.
8. Verify: `cargo test`, `cargo build`, cross-build i686-musl, mock run + screenshot.

## Open Questions

- Keep `camera`/`server` as separate libs, or fold into the `cli` crate as
  modules? Starting as separate libs; can collapse if the boundary adds no value.
- Should `/api/connect` persist the last-good host (e.g. a tiny state file) so a
  restart reconnects automatically? Deferred; in-memory for now.
