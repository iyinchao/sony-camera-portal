# Prototypes (archived — reference only)

These are the two prototypes that led to the current Rust product. They are kept
for reference and are **not built or shipped**. The canonical implementation is
the Cargo workspace under `packages/`.

## `go/`

The original Go MVP: `camera/` (UPnP DLNA client), `server/` (HTTP + `/api`
proxy, `go:embed` of the React UI), `main.go` (CLI, `--mock`). Fully working on
desktop with table-driven tests. Retired because it was never validated on
iSH/iOS and its `net/http` netpoller (epoll) is the weakest part of iSH's
emulation; Rust's blocking model proved reliable there instead.

## `rust-poc/`

The Rust proof-of-concept that validated the iOS-via-iSH path end-to-end (ran on
a real iSH install and showed live camera photos). It established the
iSH-specific techniques the product inherits: a hand-rolled blocking HTTP client
with **no socket options** (iSH rejects `setsockopt` timeouts with EINVAL),
non-blocking UDP polling, and discovery that falls back from SSDP (blocked on
iOS) to local-IP gateway probing. The product (`packages/`) is the cleaned-up,
restructured version of this.

See `openspec/changes/migrate-to-rust-product/` for the migration rationale.
