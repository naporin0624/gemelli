#!/usr/bin/env bash
# Regenerate the gemelli app icon assets from scripts/gen-icon.py:
#   crates/gui/assets/icon.png   (1024x1024 RGBA master, embedded at runtime)
#   crates/gui/assets/icon.icns  (macOS .app bundle icon)
# Requires: python3 + Pillow, and macOS `sips` + `iconutil`.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ASSETS="$ROOT/crates/gui/assets"
MASTER="$ASSETS/icon.png"

echo "Drawing master PNG…" >&2
python3 "$ROOT/scripts/gen-icon.py"

echo "Building icon.icns…" >&2
iconset="$(mktemp -d)/icon.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$MASTER" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$MASTER" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$ASSETS/icon.icns"
rm -rf "$(dirname "$iconset")"

echo "OK: $MASTER" >&2
echo "OK: $ASSETS/icon.icns" >&2
