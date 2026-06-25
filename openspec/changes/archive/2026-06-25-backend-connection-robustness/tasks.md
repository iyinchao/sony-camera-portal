## 1. Cache proxied media

- [x] 1.1 Added `cache_control: Option<String>` to `Response`; `serve_one()` emits the header when set
- [x] 1.2 `proxy()` sets `Cache-Control: public, max-age=31536000, immutable` on thumb + photo; `/api/list` stays uncached
- [x] 1.3 Test `media_is_cacheable_but_listing_is_not`

## 2. Concurrent server

- [x] 2.1 `Arc<AppState>` + `Arc<dyn AssetSource>` + `Arc<tiny_http::Server>`
- [x] 2.2 `serve()` spawns a fixed pool (`WORKERS = 4`), each looping `server.recv()` → `serve_one()`; `handle()` unchanged
- [x] 2.3 Lock scopes never span network I/O (connect locks only to commit; `list_page`/`fetch_*` stay tight). Verified: `/api/state` answered in 0.013s while a 3s connect was in flight

## 3. Bounded, supersedable connect

- [x] 3.1 Camera client: `connect_bounded()` uses `TcpStream::connect_timeout(addr, 3s)` (poll-based, iSH-safe) for all requests incl. discovery/probe + manual IP. Verified: bad-IP connect bounded at ~3.1s
- [x] 3.2 `AppState` epoch (`AtomicU64`); `begin_connect()`/`commit_connect()` — commit only if epoch is still current, else discard as superseded
- [x] 3.3 Tests `fresh_connect_records_its_error` + `superseded_connect_is_discarded`

## 4. Verify

- [x] 4.1 `cargo fmt/clippy --all-targets/test` green (camera 10 + server 11)
- [x] 4.2 i686-musl cross-build green (964 KB); confirmed no `setsockopt` socket-timeout calls (only `connect_timeout`/poll)
- [x] 4.3 Manual (mock): slow connect doesn't freeze `/api/state` (0.013s); bad-IP connect bounded ~3.1s. (On-device wrong→right-Wi-Fi reconnect + thumb-cache-on-scrollback to confirm on the camera.)
- [x] 4.4 CLAUDE.md notes the concurrent server, media caching, and bounded/supersedable connect
