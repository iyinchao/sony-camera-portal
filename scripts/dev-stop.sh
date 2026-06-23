#!/usr/bin/env bash
#
# dev-stop.sh — stop the dev environment started by dev.sh.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEV_DIR="$ROOT/.dev"
stopped=0

# Kill by recorded PID.
for name in backend vite; do
  pid_file="$DEV_DIR/$name.pid"
  if [ -f "$pid_file" ]; then
    pid="$(cat "$pid_file" 2>/dev/null || true)"
    if [ -n "${pid:-}" ] && kill "$pid" 2>/dev/null; then
      echo "stopped $name (pid $pid)"
      stopped=1
    fi
    rm -f "$pid_file"
  fi
done

# Fallback: pattern-kill in case the PIDs drifted.
if pkill -f 'target/debug/sony-camera-portal' 2>/dev/null; then stopped=1; fi
if pkill -f 'node_modules/.bin/vite' 2>/dev/null; then stopped=1; fi

if [ "$stopped" = 1 ]; then
  echo "dev environment stopped"
else
  echo "nothing was running"
fi
