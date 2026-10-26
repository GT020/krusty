#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
ARCH="${ARCH:-$(uname -m)}"
BIN="${BIN:-$ROOT/target/release/krusty}"
NAME="krusty-${VERSION}-linux-${ARCH}"
OUT="$ROOT/dist/$NAME"

rm -rf "$OUT"
mkdir -p "$OUT/icons/hicolor/512x512/apps"

install -Dm755 "$BIN" "$OUT/krusty"
install -Dm644 assets/krusty.desktop "$OUT/io.github.GT020.Krusty.desktop"
install -Dm644 assets/app-icon.png "$OUT/icons/hicolor/512x512/apps/io.github.GT020.Krusty.png"

tar -C "$ROOT/dist" -czf "$ROOT/dist/$NAME.tar.gz" "$NAME"
cp "$BIN" "$ROOT/dist/krusty-linux-${ARCH}"
