#!/usr/bin/env bash
# Rebuild and restart krusty whenever Rust/Toml sources change.
#
# Usage:
#   ./scripts/dev.sh            # auto: cargo-watch, then watchexec, then polling
#   ./scripts/dev.sh --release  # forwarded to `cargo run`
#   KRUSTY_DEV_WATCHER=poll ./scripts/dev.sh
#   cargo dev                   # same as the default auto mode (needs cargo-watch)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WATCH_PATHS=(src Cargo.toml Cargo.lock .cargo)
MODE="${KRUSTY_DEV_WATCHER:-auto}"

has() { command -v "$1" >/dev/null 2>&1; }

if [[ "$MODE" == "auto" ]]; then
  if cargo watch --version >/dev/null 2>&1; then
    MODE=watch
  elif has watchexec; then
    MODE=watchexec
  else
    MODE=poll
  fi
fi

PROFILE=debug
for arg in "$@"; do
  [[ "$arg" == "--release" ]] && PROFILE=release
done

case "$MODE" in
  watch)
    args=()
    for path in "${WATCH_PATHS[@]}"; do args+=(-w "$path"); done
    for arg in "$@"; do
      [[ "$arg" == "--release" ]] && exec cargo watch "${args[@]}" --exec "run --release"
    done
    exec cargo watch "${args[@]}" --exec run
    ;;
  watchexec)
    args=()
    for path in "${WATCH_PATHS[@]}"; do args+=(-w "$path"); done
    exec watchexec --restart --clear "${args[@]}" -- cargo run -- "$@"
    ;;
  poll)
    snapshot() {
      find "${WATCH_PATHS[@]}" -type f \( -name '*.rs' -o -name '*.toml' \) \
        -printf '%T@ %p\n' 2>/dev/null | sort | md5sum
    }
    app_pid=""
    cleanup() {
      [[ -n "$app_pid" ]] && kill "$app_pid" 2>/dev/null || true
      wait 2>/dev/null || true
    }
    trap cleanup EXIT INT TERM
    while true; do
      if cargo build "$@"; then
        "./target/$PROFILE/krusty" &
        app_pid=$!
      else
        app_pid=""
      fi
      last="$(snapshot)"
      while [[ "$(snapshot)" == "$last" ]]; do sleep 1; done
      cleanup
      app_pid=""
      echo "change detected, rebuilding..."
    done
    ;;
  *)
    echo "unknown KRUSTY_DEV_WATCHER=$MODE (use auto|watch|watchexec|poll)" >&2
    exit 2
    ;;
esac
