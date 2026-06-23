# scp-poc — Rust proof-of-concept

A self-contained Rust port of the sony-camera-portal core, built to answer one
question: **can we ship a single static binary that runs on iSH (iOS)?**

It mirrors the Go implementation — SSDP discovery → UPnP ContentDirectory
`Browse` → typed photos → a blocking `tiny_http` server that proxies
`/api/list`, `/api/thumb/:id`, `/api/photo/:id` and serves a minimal grid page.

Deliberately **blocking + single-threaded (no tokio)**, which suits iSH's
emulated environment. The web page is a tiny inline HTML grid — this PoC does
**not** depend on the React `web/` frontend, so no npm is involved.

## Why this exists / status

- ✅ Builds to `i686-unknown-linux-musl`: **static 32-bit ELF, ~887 KB**
  (vs the Go binary's ~9.3 MB).
- ✅ Parsing verified against the real captured fixtures (`--selftest`).
- ⏳ Pending: on-device test in iSH against a live camera (the actual proof).

See `../openspec/` and the project memory `go-on-ish-ios` for the iOS strategy
discussion (Rust vs Go-on-iSH).

## Run (development)

The PoC serves its own inline gallery page, so a dev run is a single
`cargo run` — no Vite, no npm.

```sh
cd rust-poc

# ① Preview the UI with no camera (SVG placeholder thumbnails) — daily dev
cargo run -- --mock 24
#   → open http://127.0.0.1:8080

# ② Verify parsing offline against the captured fixtures (no camera, no network)
cargo run -- --selftest ../camera/testdata

# ③ Against a real camera (must be joined to the camera's Wi-Fi AP)
cargo run -- --camera-host 10.0.0.1
#   → open http://127.0.0.1:8080

# Custom port
cargo run -- --mock 24 --port 9000
```

Edit `src/main.rs` and re-run `cargo run` (incremental builds are ~1 s).

| Mode | Command | Needs camera? |
|------|---------|---------------|
| mock | `cargo run -- --mock 24` | no |
| selftest | `cargo run -- --selftest ../camera/testdata` | no |
| live | `cargo run -- --camera-host 10.0.0.1` | yes |

## Flags

| Flag | Default | Meaning |
|------|---------|---------|
| `--port` | `8080` | localhost port (binds `127.0.0.1`) |
| `--camera-host` | _(SSDP discover)_ | camera IP; builds `http://HOST:64321/DmsDesc.xml` |
| `--mock N` | _(off)_ | serve N synthetic photos with SVG placeholders |
| `--selftest DIR` | _(off)_ | parse `DIR/DmsDesc.xml` + `DIR/browse_response.xml`, print, exit |

## Cross-compile for iSH (iOS)

Needs the musl target and a cross-linker. We use `cargo-zigbuild` (Zig as the
musl linker — no separate gcc toolchain):

```sh
brew install zig
cargo install cargo-zigbuild
rustup target add i686-unknown-linux-musl

cargo zigbuild --release --target i686-unknown-linux-musl
# → target/i686-unknown-linux-musl/release/scp-poc  (static i386, ~887 KB)
```

Verify it's a static 32-bit ELF:

```sh
file target/i686-unknown-linux-musl/release/scp-poc
# ELF 32-bit LSB executable, Intel 80386, ... statically linked, stripped
```

## Run it on iSH (on-device test)

Prepare **before** joining the camera (the camera AP has no internet):

1. Install **iSH** from the App Store.
2. Get `scp-poc` into iSH: AirDrop the binary to the iPhone, save it into iSH's
   folder via the Files app (iSH shows up as a location in Files).
3. In iSH: `chmod +x scp-poc`

Then:

4. Join the camera's Wi-Fi (Send to Smartphone → Select on Smartphone).
5. In iSH: `./scp-poc --camera-host 10.0.0.1` (allow the Local Network prompt).
6. Open Safari to `http://localhost:8080`.
7. Keep iSH alive while in Safari: iPad Split View, or enable Location Services
   for iSH on iPhone.

> Unlike Go, this Rust binary needs no `GOMAXPROCS=1` workaround — it has no GC /
> goroutine scheduler, so it sidesteps the iSH threading race that hangs Go.

## Dependencies

Kept pure-Rust so the musl cross-build stays clean (no C/asm like ring):

- `tiny_http` — blocking HTTP server
- `ureq` (default-features off → http-only, no TLS)
- `roxmltree` — XML parsing
- `serde_json` — JSON output

> Note: `ureq` pulls in `url`/`idna`/`icu`. To shrink the binary further, a
> lighter HTTP client is a future option; 887 KB is already fine.
