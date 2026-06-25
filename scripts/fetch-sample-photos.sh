#!/usr/bin/env bash
# Download sample photos at varied resolutions / aspect ratios for UI dev.
# Source: picsum.photos (deterministic via /seed/<seed>/<w>/<h>).
# Usage: scripts/fetch-sample-photos.sh [COUNT] [OUTDIR]
#   COUNT  number of photos to fetch (default 24)
#   OUTDIR target directory (default .dev/sample-photos)
set -euo pipefail

COUNT="${1:-24}"
OUTDIR="${2:-.dev/sample-photos}"

# Common photo aspect ratios, scaled to web-friendly long edges so the gallery
# has a real mix of landscape / portrait / square / panorama tiles.
#   w:h pairs — 3:2 & 2:3 are the a6000's native still ratio.
RATIOS=(
  3000x2000  # 3:2 landscape
  2000x3000  # 2:3 portrait
  3840x2160  # 16:9
  1080x1920  # 9:16 portrait
  2000x2000  # 1:1 square
  2048x1536  # 4:3
  1536x2048  # 3:4 portrait
  4000x1333  # ~3:1 panorama
  2400x1600  # 3:2 smaller
)

mkdir -p "$OUTDIR"
echo "Fetching $COUNT photos into $OUTDIR ..."

for i in $(seq 0 $((COUNT - 1))); do
  dim="${RATIOS[$((i % ${#RATIOS[@]}))]}"
  w="${dim%x*}"; h="${dim#*x}"
  out="$OUTDIR/$(printf 'DSC%05d_%sx%s.jpg' $((7000 + i)) "$w" "$h")"
  url="https://picsum.photos/seed/sony${i}/${w}/${h}"
  # -L follow redirect, --fail on HTTP errors, retry transient failures.
  if curl -fsSL --retry 3 --retry-delay 1 -o "$out" "$url"; then
    printf '  [%2d/%d] %s\n' "$((i + 1))" "$COUNT" "$(basename "$out")"
  else
    echo "  ! failed: $url" >&2
  fi
done

echo "Done. $(ls -1 "$OUTDIR" | wc -l | tr -d ' ') files in $OUTDIR"
