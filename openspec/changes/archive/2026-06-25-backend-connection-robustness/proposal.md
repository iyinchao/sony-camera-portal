## Why

Real-camera testing surfaced three backend issues, all rooted in the server being
single-threaded with unbounded blocking I/O and no proxy caching:

1. **Thumbnails re-fetch on every re-mount.** The `/api/thumb` and `/api/photo`
   proxy responses carry no `Cache-Control`, so the browser never caches them.
   With the virtualized gallery, scrolling away unmounts a tile's `<img>` and
   scrolling back re-fetches it from the camera — slow over the camera Wi-Fi.
2. **A `/api/connect` freezes the whole UI.** `serve()` handles requests one at a
   time, so while a connect is doing its (blocking) discovery, every other request
   — `/api/state`, thumbnails, static assets — is stuck behind it.
3. **A stuck connect blocks a new one.** If the user is on the wrong Wi-Fi, the
   connect blocks on an unreachable gateway probe (no connect timeout, ~tens of
   seconds). Switching to the right Wi-Fi and refreshing does not reconnect
   promptly — the new attempt queues behind the dead one, and a late-finishing
   stale attempt can clobber the new state.

## What Changes

- **Cache proxied media.** `/api/thumb/:id` and `/api/photo/:id` responses get
  `Cache-Control: max-age=…, immutable` (content is fixed per id), so the browser
  serves re-mounted images from its own cache instead of re-hitting the camera.
- **Concurrent server.** `serve()` becomes multi-threaded (a small worker pool
  over a shared `tiny_http::Server`); `AppState` and the asset source are shared
  via `Arc`. A slow `/api/connect` no longer blocks `/api/state`, media, or
  assets (the connect's slow discovery already runs without holding the state
  lock).
- **Bounded, supersedable connect.**
  - Discovery/probe connects use a **bounded connect timeout** (`connect_timeout`,
    which polls rather than using the `setsockopt` timeouts iSH rejects) so an
    unreachable host fails in ~1–2s instead of hanging.
  - A **connect epoch** in `AppState`: starting a connect bumps the epoch; when an
    attempt finishes it only commits its result (success or error) if its epoch is
    still current, so a superseded/stale attempt can't overwrite newer state.
- **Reuse on refresh is preserved/clarified.** An already-connected camera is
  reused on reload (the UI's bootstrap already does this); concurrency just makes
  it actually observable when a prior attempt is still settling.

## Capabilities

### Modified Capabilities
- `connection-manager`: connecting is concurrent (doesn't freeze the server),
  bounded (no indefinite hang), and supersedable (a newer connect wins; stale
  results are discarded), and proxied media is browser-cacheable.

## Impact

- Backend only: `packages/server` (`serve()` threading, `Response` cache header,
  `AppState` epoch) and `packages/camera` (bounded connect in discovery/probe).
- No frontend change required; the cache + concurrency are transparent. (The UI's
  existing reconnect/bootstrap behavior benefits automatically.)
- `cargo fmt/clippy/test` stays green; must still cross-compile to
  `i686-unknown-linux-musl` (iSH) — so the connect timeout must avoid the
  `setsockopt` socket timeouts iSH rejects (use `connect_timeout`'s poll path).
- Threads must work under iSH's emulation (std threads on musl are fine; use a
  small fixed pool, not unbounded spawn).
