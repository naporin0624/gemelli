#!/usr/bin/env bash
# Fetch the Spout2 SDK (BSD-2-Clause) into vendor/Spout2/ for the gemelli-spout
# native bridge — crates/spout/build.rs compiles SpoutDX + SpoutGL from here.
# Mirrors scripts/fetch-fonts.sh; vendor/Spout2 is gitignored. Required before
# `cargo build -p gemelli-spout` on Windows.
set -euo pipefail

TAG="2.007.017"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/vendor/Spout2"
URL="https://github.com/leadedge/Spout2/archive/refs/tags/${TAG}.tar.gz"

tmp="$ROOT/_spout2_tmp"
rm -rf "$tmp" "$DEST"
mkdir -p "$tmp" "$DEST/SpoutDirectX"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading Spout2 ${TAG} from $URL" >&2
if ! curl -fsSL "$URL" -o "$tmp/spout2.tar.gz"; then
  echo "ERROR: failed to download $URL" >&2
  exit 1
fi

tar -xzf "$tmp/spout2.tar.gz" -C "$tmp"
src="$tmp/Spout2-${TAG}/SPOUTSDK"
if [ ! -f "$src/SpoutDirectX/SpoutDX/SpoutDX.cpp" ] || [ ! -d "$src/SpoutGL" ]; then
  echo "ERROR: expected SDK layout not found under $src" >&2
  exit 1
fi

# Preserve the SDK's directory structure: SpoutDX.h references ../../SpoutGL/.
cp -R "$src/SpoutDirectX/SpoutDX" "$DEST/SpoutDirectX/SpoutDX"
cp -R "$src/SpoutGL" "$DEST/SpoutGL"
# Keep the license next to the vendored source (referenced by THIRD-PARTY-NOTICES).
cp "$tmp/Spout2-${TAG}/LICENSE" "$DEST/LICENSE" 2>/dev/null || true

echo "Spout2 SDK ${TAG} fetched to $DEST" >&2
