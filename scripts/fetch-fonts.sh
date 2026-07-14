#!/usr/bin/env bash
# Fetch LINE Seed JP (SIL OFL 1.1) from the official line/seed release into vendor/fonts/,
# for embedding into the gemelli-gui binary via include_bytes! (crates/gui/src/fonts.rs).
# Required before `cargo build -p gemelli-gui` — see crates/gui/src/fonts.rs for the
# compile-time dependency this creates.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/vendor/fonts"
URL="https://github.com/line/seed/releases/download/v20251119/seed-v20251119.zip"

mkdir -p "$DEST"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading LINE Seed JP from $URL" >&2
if ! curl -fsSL "$URL" -o "$tmp/seed.zip"; then
  echo "ERROR: failed to download $URL" >&2
  exit 1
fi

# Prefer unzip when present; Git Bash on windows-latest doesn't reliably ship it, but
# windows-latest's bsdtar (invoked as `tar`) can extract zips too.
extract_zip() {
  archive="$1"
  dest="$2"
  mkdir -p "$dest"
  if command -v unzip >/dev/null 2>&1; then
    unzip -q "$archive" -d "$dest"
  else
    tar -xf "$archive" -C "$dest"
  fi
}

if ! extract_zip "$tmp/seed.zip" "$tmp/x"; then
  echo "ERROR: failed to unzip the downloaded archive ($tmp/seed.zip)" >&2
  exit 1
fi

# Some LINE Seed releases nest per-language/weight fonts inside inner zips; expand any
# that are present so the font search below sees everything. This particular release
# (v20251119) does not nest, but future releases might, so this stays defensive.
find "$tmp/x" -name '*.zip' -print0 | while IFS= read -r -d '' inner; do
  extract_zip "$inner" "${inner}.d" || true
done

# Prefer an actual .ttf; only fall back to .otf if no ttf match exists.
font="$(find "$tmp/x" -type f \
  \( -iname '*JP*Rg*.ttf' -o -iname '*JP*Regular*.ttf' \) -print -quit)"
if [ -z "$font" ]; then
  font="$(find "$tmp/x" -type f \
    \( -iname '*JP*Rg*.otf' -o -iname '*JP*Regular*.otf' \) -print -quit)"
fi
if [ -z "$font" ]; then
  echo "ERROR: no LINE Seed JP Regular font (ttf or otf) found in the archive." >&2
  echo "Inspect the archive layout (unzip -l $tmp/seed.zip) and adjust the find globs in this script." >&2
  exit 1
fi
cp "$font" "$DEST/LINESeedJP-Regular.ttf"

license="$(find "$tmp/x" -type f \( -iname 'OFL*' -o -iname 'LICENSE*' \) -print -quit)"
if [ -z "$license" ]; then
  echo "ERROR: no OFL/LICENSE file found in the archive." >&2
  exit 1
fi
cp "$license" "$DEST/LICENSE"

echo "OK: $DEST/LINESeedJP-Regular.ttf ($(wc -c < "$DEST/LINESeedJP-Regular.ttf") bytes)" >&2
echo "OK: $DEST/LICENSE" >&2
