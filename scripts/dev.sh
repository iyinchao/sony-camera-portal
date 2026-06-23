#!/usr/bin/env bash
#
# dev.sh — start the development environment, detached:
#   - Rust backend with mock data on 127.0.0.1:8080 (serves /api)
#   - Vite dev server on http://localhost:5173 (HMR; proxies /api → :8080)
#
# Both keep running after this script exits (nohup). Stop with dev-stop.sh.
#
# Usage:  ./scripts/dev.sh [MOCK_COUNT]      # default 24 fake photos
#
# Open http://localhost:5173 and edit packages/web/src/* for live reload.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
MOCK="${1:-24}"
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

echo "==> Starting Rust backend (mock $MOCK) on 127.0.0.1:$PORT …"
nohup target/debug/sony-camera-portal --port "$PORT" --mock "$MOCK" --no-open \
  >"$DEV_DIR/backend.log" 2>&1 &
echo $! >"$DEV_DIR/backend.pid"

echo "==> Starting Vite dev server …"
(
  cd packages/web
  nohup node_modules/.bin/vite >"$DEV_DIR/vite.log" 2>&1 &
  echo $! >"$DEV_DIR/vite.pid"
)

# Give them a moment, then health-check.
sleep 2
backend_ok=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "http://127.0.0.1:$PORT/api/state" || echo 000)
vite_ok=$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "http://localhost:5173/" || echo 000)

echo ""
echo "Dev environment:"
echo "  Frontend (HMR):   http://localhost:5173    ← open this   [$vite_ok]"
echo "  Backend  (mock):  http://127.0.0.1:$PORT                 [$backend_ok]"
echo "  Logs:             .dev/backend.log   .dev/vite.log"
echo "  Stop:             ./scripts/dev-stop.sh"

if [ "$backend_ok" != 200 ] || [ "$vite_ok" != 200 ]; then
  echo ""
  echo "⚠️  Something didn't come up cleanly — check the logs above."
fi
