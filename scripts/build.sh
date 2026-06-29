#!/usr/bin/env bash
#
# build.sh — build a release binary for one or more target platforms and
# collect it into dist/ under a per-platform name.
#
# Mock code is stripped (--no-default-features). The web bundle is built once
# and embedded. Builders:
#   zig   — cargo-zigbuild, static musl (no libc/NDK needed). iSH / Linux.
#   ndk   — Android NDK clang linker → PIE Bionic binary (Android REQUIRES PIE,
#           so static musl is rejected with "unexpected e_type: 2"). Termux.
#   cargo — native host build. macOS.
#
# Usage:  ./scripts/build.sh <platform|clean> [platform...]
#   ish         iSH / iOS          i686-unknown-linux-musl         (zig, static)
#   android     Termux arm64       aarch64-linux-android           (ndk, PIE)
#   android32   Termux armv7       armv7-linux-androideabi         (ndk, PIE)
#   linux       Linux x86_64       x86_64-unknown-linux-musl       (zig, static)
#   linux-arm   Linux arm64        aarch64-unknown-linux-musl      (zig, static)
#   windows     Windows x86_64     x86_64-pc-windows-gnu           (zig)
#   macos       macOS (host arch)  native                          (cargo)
#   all         = ish android linux macos
#   clean       remove dist/ and the cross-compile target dirs (keeps the
#               host debug build dev.sh uses; `cargo clean` resets everything)
#
# Android needs the NDK: set ANDROID_NDK_HOME, or have it under
# $ANDROID_HOME/ndk/* (auto-detected; latest version used). NDK_API sets the
# min Android API level (default 24 = Android 7).
# zig builders need: `cargo install cargo-zigbuild` + zig.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DIST="$ROOT/dist"
NDK_API="${NDK_API:-24}"

# platform → "<rust-target> <builder>"
target_for() {
  case "$1" in
    ish) echo "i686-unknown-linux-musl zig" ;;
    android) echo "aarch64-linux-android ndk" ;;
    android32) echo "armv7-linux-androideabi ndk" ;;
    linux) echo "x86_64-unknown-linux-musl zig" ;;
    linux-arm) echo "aarch64-unknown-linux-musl zig" ;;
    windows) echo "x86_64-pc-windows-gnu zig" ;;
    macos) echo "$(rustc -vV | sed -n 's/host: //p') cargo" ;;
    *) echo "" ;;
  esac
}

# platform → dist filename suffix (friendly, distributable name).
suffix_for() {
  case "$1" in
    ish) echo "ish-i686" ;;
    android) echo "android-arm64" ;;
    android32) echo "android-armv7" ;;
    linux) echo "linux-x86_64" ;;
    linux-arm) echo "linux-arm64" ;;
    windows) echo "windows-x86_64.exe" ;;
    macos) case "$(uname -m)" in arm64 | aarch64) echo "macos-arm64" ;; *) echo "macos-x86_64" ;; esac ;;
  esac
}

# The NDK clang wrapper prefix differs from the rust triple for armv7.
ndk_clang_prefix() {
  case "$1" in
    armv7-linux-androideabi) echo "armv7a-linux-androideabi" ;;
    *) echo "$1" ;;
  esac
}

# Locate an Android NDK (env, then common SDK locations; latest version).
find_ndk() {
  local c sdk
  for c in "${ANDROID_NDK_HOME:-}" "${ANDROID_NDK_ROOT:-}"; do
    [ -n "$c" ] && [ -d "$c" ] && {
      echo "$c"
      return
    }
  done
  for sdk in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" "$HOME/Library/Android/sdk" "$HOME/Android/Sdk"; do
    [ -n "$sdk" ] && [ -d "$sdk/ndk" ] && {
      ls -d "$sdk"/ndk/*/ 2>/dev/null | sort -V | tail -1 | sed 's#/$##'
      return
    }
  done
}

usage() {
  sed -n '3,35p' "$0" | sed 's/^#\{0,1\} \{0,1\}//'
  exit 1
}

[ $# -ge 1 ] || usage

# `clean`: drop dist/ and the cross-compile target dirs (the host target dir is
# left alone so dev.sh's debug build survives).
if [ "${1:-}" = clean ]; then
  echo "==> cleaning…"
  [ -d "$DIST" ] && rm -rf "$DIST" && echo "    removed dist/"
  for plat in ish android android32 linux linux-arm windows; do
    spec="$(target_for "$plat")"
    t="${spec% *}"
    [ -d "target/$t" ] && rm -rf "target/$t" && echo "    removed target/$t"
  done
  echo "==> done (host debug build kept; run 'cargo clean' to reset everything)."
  exit 0
fi

# Expand 'all'.
PLATFORMS=()
for p in "$@"; do
  if [ "$p" = all ]; then PLATFORMS+=(ish android linux macos); else PLATFORMS+=("$p"); fi
done

echo "==> Building web bundle…"
(
  cd packages/web
  [ -d node_modules ] || npm ci
  npm run build >/dev/null
)
echo "    web bundle ready (packages/web/dist)"
echo ""

mkdir -p "$DIST"

FAILED=()
for plat in "${PLATFORMS[@]}"; do
  spec="$(target_for "$plat")"
  if [ -z "$spec" ]; then
    echo "!! unknown platform: $plat (skipping)" >&2
    FAILED+=("$plat")
    continue
  fi
  target="${spec% *}"
  builder="${spec#* }"
  echo "==> $plat  →  $target  ($builder, release, no mock)"
  # Install the rust target if missing — visibly, since the std download can take
  # a while and a silent wait looks like a hang.
  if ! rustup target list --installed 2>/dev/null | grep -qx "$target"; then
    echo "    (installing rust target $target — downloading std…)"
    rustup target add "$target"
  fi

  case "$builder" in
    zig)
      cargo zigbuild --release --target "$target" --no-default-features
      ;;
    cargo)
      cargo build --release --target "$target" --no-default-features
      ;;
    ndk)
      ndk="$(find_ndk)"
      if [ -z "$ndk" ]; then
        echo "!! Android NDK not found — set ANDROID_NDK_HOME or install it under \$ANDROID_HOME/ndk" >&2
        FAILED+=("$plat")
        echo ""
        continue
      fi
      tcbin="$(ls -d "$ndk"/toolchains/llvm/prebuilt/*/bin 2>/dev/null | head -1)"
      cc="$tcbin/$(ndk_clang_prefix "$target")$NDK_API-clang"
      if [ ! -x "$cc" ]; then
        echo "!! NDK clang not found: $cc" >&2
        FAILED+=("$plat")
        echo ""
        continue
      fi
      linker_var="CARGO_TARGET_$(echo "$target" | tr 'a-z-' 'A-Z_')_LINKER"
      echo "    NDK: $ndk (API $NDK_API)"
      env "$linker_var=$cc" "AR_$(echo "$target" | tr - _)=$tcbin/llvm-ar" \
        cargo build --release --target "$target" --no-default-features
      ;;
  esac

  bin="target/$target/release/sony-camera-portal"
  [ "$plat" = windows ] && bin="$bin.exe"
  if [ ! -f "$bin" ]; then
    echo "!! expected binary not found: $bin" >&2
    FAILED+=("$plat")
    echo ""
    continue
  fi

  out="$DIST/sony-camera-portal-$(suffix_for "$plat")"
  cp -f "$bin" "$out"
  printf "    ✓ dist/%s  (%s)\n" "$(basename "$out")" "$(ls -lh "$out" | awk '{print $5}')"
  echo ""
done

if [ ${#FAILED[@]} -gt 0 ]; then
  echo "==> done with failures: ${FAILED[*]}" >&2
  exit 1
fi
echo "==> done. Binaries collected in dist/:"
ls -1 "$DIST"
