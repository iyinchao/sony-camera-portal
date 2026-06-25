## Context

The localhost server (`packages/server`) serves the embedded web UI and proxies
`/api` to the connected camera. Today `serve()` is a single-threaded
`for request in server.incoming_requests()` loop, `AppState` is a `Mutex<Inner>`,
and `connect()` runs blocking discovery (which already does NOT hold the lock
during the slow part — it locks only to swap the source). The camera client
(`packages/camera`) uses a hand-rolled blocking HTTP client over `TcpStream` with
no socket options (iSH rejects `setsockopt` timeouts with EINVAL). This change
makes the server concurrent, caches proxied media, and makes connecting bounded
and supersedable — without breaking the iSH build.

## Goals / Non-Goals

**Goals:**
- A slow/failed connect never freezes `/api/state`, media, or static assets.
- Re-mounted thumbnails come from the browser cache, not a re-fetch.
- A newer connect supersedes a stale in-flight one; stale results don't clobber
  newer state. An already-connected camera is reused on refresh.
- Discovery/probe connects are bounded (~1–2s), not indefinite.
- iSH stays buildable/runnable: no `setsockopt` socket timeouts; bounded threads.

**Non-Goals:**
- Read/response timeouts on the camera client (the reported hang is on *connect*;
  full non-blocking read timeouts are a deeper follow-up, noted in Risks).
- True cancellation of an in-flight blocking syscall (we supersede by epoch +
  bound the connect so stale attempts end quickly, rather than hard-killing them).
- Any frontend change.

## Decisions

- **Cache-Control on proxied media.** Add an optional `cache_control: Option<String>`
  to `Response`; `proxy()` sets `Cache-Control: public, max-age=31536000, immutable`
  for thumb and photo (content is immutable per camera object id). `serve()` emits
  the header when present. Localhost single-user, so a long max-age is safe; a new
  camera uses new ids, and `/api/list` responses stay uncached.
- **Concurrent server via a fixed worker pool.** Wrap the server in `Arc`, share
  `Arc<AppState>` and `Arc<dyn AssetSource>`, and spawn a small fixed number of
  worker threads (e.g. `N = 4`), each looping `server.recv()` → `handle()` →
  `respond()`. Fixed pool (not thread-per-request) bounds resource use on iSH.
  `handle()` is already pure over `&AppState`/`&dyn AssetSource`, so only wiring
  changes. The connect's slow discovery holds no lock, so other workers proceed.
- **Connect epoch (supersede).** `AppState` gains an epoch counter (e.g.
  `AtomicU64`). `connect()` reads-and-bumps the epoch at entry to get `my_epoch`;
  after discovery finishes it locks `Inner` and commits its result **only if the
  epoch is unchanged** (no newer connect started); otherwise it discards (returns
  a "superseded" outcome without touching `source`/`last_error`). This prevents a
  late stale failure from clearing a newer success (and vice-versa).
- **Bounded connect timeout (iSH-safe).** In the camera client, replace
  `TcpStream::connect(addr)` with `TcpStream::connect_timeout(&addr, 3s)` in the
  discovery/probe path (and the manual-IP connect). `connect_timeout` uses
  non-blocking connect + `poll` (no `SO_SNDTIMEO`/`SO_RCVTIMEO`), so it works on
  iSH. 3s is generous for a local network (a real camera, being the AP gateway,
  answers in milliseconds, so the timeout rarely fires) yet bounds an unreachable
  host. Caveat: discovery probes candidates **sequentially**, so a fully-failed
  auto-discover costs ≈ `candidates × 3s` (~3 → ~9s worst case); on the camera AP
  the first candidate answers instantly so this never bites. Parallelizing the
  candidate probes (now that the server is threaded) could cap total discovery at
  ~3s — noted as an optional follow-up, not required here.
- **Reuse on refresh.** Unchanged behavior, now observable: the UI bootstrap calls
  `/api/state` first and only connects when not already connected. With the
  server concurrent, that `/api/state` is answered immediately even while a prior
  attempt settles.

## Risks / Trade-offs

- [Shared mutable state across threads] → `AppState` is already `Mutex`-based and
  `Send + Sync`; wrap in `Arc`. Keep lock scopes tiny (never hold across network
  I/O — already true in `connect()`; preserve that in `list_page`/`fetch_*`).
- [`connect_timeout` and address resolution] → resolve the host:port to a
  `SocketAddr` first (the camera host is an IP, so resolution is trivial/local).
- [Read still unbounded] → a camera that accepts but never responds still blocks
  that one worker's read; bounded connect covers the reported case, and the pool
  means one stuck read doesn't freeze the server. Note read-timeouts as a future
  hardening (non-blocking + `poll`, also iSH-safe).
- [Long cache could serve a stale thumb if a camera reused an id for new content]
  → camera object ids are stable per asset; acceptable. If ever a problem, scope
  the max-age to the session.
- [Thread count on iSH] → fixed small pool (4); avoid unbounded spawn.

## Open Questions

- Worker count: fixed 4, or `min(4, available_parallelism)`? Default to a small
  constant for predictability on iSH.
- Should `/api/connect` return a distinct "superseded" status, or just the current
  state? Lean on returning current state (the UI already polls `/api/state`).
