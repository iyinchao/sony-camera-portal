#!/usr/bin/env bash
#
# dev.sh — start the development environment, detached:
#   - Rust backend on 127.0.0.1:8080 (serves /api), mock data OR a real camera
#   - Vite dev server on http://localhost:5173 (HMR; proxies /api → :8080)
#
# Both keep running after this script exits (nohup). Stop with dev-stop.sh.
#
# Usage:  ./scripts/dev.sh [MOCK_COUNT|real]
#   ./scripts/dev.sh           # default: mock 24 fake photos
#   ./scripts/dev.sh 1000      # mock 1000 fake photos
#   ./scripts/dev.sh real      # no mock — connect a real camera from the web UI
#                              # (aliases: none | off | no | --no-mock)
#
# Open http://localhost:5173 and edit packages/web/src/* for live reload.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# Mock vs real camera. Numeric arg → that many mock photos; 'real' (and aliases)
# → start with no mock so a real camera is connected from the web UI.
ARG="${1:-24}"
case "$ARG" in
  real | none | off | no | --no-mock | --real)
    MOCK_ARGS=()
    MODE_DESC="real camera (no mock — connect from the web UI)"
    ;;
  '' | *[!0-9]*)
    echo "Usage: ./scripts/dev.sh [MOCK_COUNT|real]" >&2
    echo "  numeric = mock that many photos (default 24); 'real' = no mock" >&2
    exit 1
    ;;
  *)
    MOCK_ARGS=(--mock "$ARG")
    MODE_DESC="mock $ARG photos"
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
