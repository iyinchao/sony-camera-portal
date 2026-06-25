#!/usr/bin/env bash
#
# dev.sh — start the development environment, detached:
#   - Rust backend on 127.0.0.1:8080 (serves /api), mock data OR a real camera
#   - Vite dev server on http://localhost:5173 (HMR; proxies /api → :8080)
#
# Both keep running after this script exits (nohup). Stop with dev-stop.sh.
#
# Usage:  ./scripts/dev.sh [COUNT|real] [DIR] [DELAY_SECS]
#   ./scripts/dev.sh                       # default: 100 synthetic photos
#   ./scripts/dev.sh 1000                  # 1000 synthetic photos
#   ./scripts/dev.sh 100 ./demo-images     # 100 photos, images cycled from the dir
#   ./scripts/dev.sh 100 ./demo-images 5   # …with a 5s simulated connect delay
#   ./scripts/dev.sh real                  # no mock — connect a real camera
#                                          # (aliases: none | off | no | --no-mock)
#
# Open http://localhost:5173 and edit packages/web/src/* for live reload.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Mock vs real camera. COUNT is the photo count (synthetic, or how many to draw
# from DIR — cycling if COUNT exceeds the file count); 'real' (and aliases) → no
# mock. DIR (optional) sources real images; DELAY (optional) simulates connect
# latency in seconds.
ARG="${1:-100}"
DIR="${2:-}"
DELAY="${3:-0}"
case "$ARG" in
  real | none | off | no | --no-mock | --real)
    MOCK_ARGS=()
    MODE_DESC="real camera (no mock — connect from the web UI)"
    ;;
  *)
    MOCK_ARGS=(--mock "$ARG")
    MODE_DESC="mock $ARG photos"
    if [ -n "$DIR" ]; then
      MOCK_ARGS+=(--mock-dir "$DIR")
      MODE_DESC="$MODE_DESC from '$DIR'"
    fi
    if [ "$DELAY" != 0 ]; then
      MOCK_ARGS+=(--mock-delay "$DELAY")
      MODE_DESC="$MODE_DESC, ${DELAY}s connect delay"
    fi
    ;;
esac

# Backend port is fixed to 8080 because vite.config.ts proxies /api there.
PORT=8080
DEV_DIR="$ROOT/.dev"
mkdir -p "$DEV_DIR"

# Free the ports first (idempotent restart).
"$ROOT/scripts/dev-stop.sh" >/dev/null 2>&1 || true

echo "==> Building backend (cargo)…"
cargo build -q

echo "==> Checking frontend deps…"
[ -d packages/web/node_modules ] || (cd packages/web && npm ci)

echo "==> Starting Rust backend ($MODE_DESC) on 127.0.0.1:$PORT …"
nohup target/debug/sony-camera-portal --port "$PORT" \
  ${MOCK_ARGS[@]+"${MOCK_ARGS[@]}"} --no-open \
  >"$DEV_DIR/backend.log" 2>&1 &
echo $! >"$DEV_DIR/backend.pid"

echo "==> Starting Vite dev server …"
(
  cd packages/web
  nohup node_modules/.bin/vite >"$DEV_DIR/vite.log" 2>&1 &
  echo $! >"$DEV_DIR/vite.pid"
)

# Give them a moment, then health-check. --noproxy so a local HTTP proxy
# (http_proxy / Whistle / etc.) doesn't intercept the localhost check.
sleep 2
backend_ok=$(curl -s --noproxy '*' -o /dev/null -w '%{http_code}' --max-time 3 "http://127.0.0.1:$PORT/api/state" || echo 000)
vite_ok=$(curl -s --noproxy '*' -o /dev/null -w '%{http_code}' --max-time 3 "http://localhost:5173/" || echo 000)

echo ""
echo "Dev environment:"
echo "  Frontend (HMR):   http://localhost:5173    ← open this   [$vite_ok]"
echo "  Backend:          http://127.0.0.1:$PORT  ($MODE_DESC)  [$backend_ok]"
echo "  Logs:             .dev/backend.log   .dev/vite.log"
echo "  Stop:             ./scripts/dev-stop.sh"

if [ "$backend_ok" != 200 ] || [ "$vite_ok" != 200 ]; then
  echo ""
  echo "⚠️  Something didn't come up cleanly — check the logs above."
fi
