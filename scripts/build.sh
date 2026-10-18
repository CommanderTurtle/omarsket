#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:?plugin or runner required}"
shift
[[ "$MODE" == plugin || "$MODE" == runner ]] || { echo 'Expected plugin or runner' >&2; exit 1; }
if [[ ${1:-} == --help ]]; then
  printf 'Usage: bash build-%s.sh\nBuild the Rust engine and initialize its private Python environment.\nPlugin add/enable/update/remove are handled by omarchy plugin, not this script.\n' "$MODE"
  exit 0
fi
(( $# == 0 )) || { echo 'No install flags. Install dependencies first; use omarchy plugin to manage the plugin.' >&2; exit 1; }
[[ "$(uname -s)" == Linux ]] || { echo 'Initialize on Linux (Omarchy/Hyprland).' >&2; exit 1; }
for tool in cargo cc uv python3 git; do
  command -v "$tool" >/dev/null || { printf 'Missing %s; install the prerequisites in README.md.\n' "$tool" >&2; exit 1; }
done

# Keep build products and Python symlinks outside Omarchy's Git-managed plugin.
CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/omarsket"
OUT="${XDG_DATA_HOME:-$HOME/.local/share}/omarsket/runtime"
[[ "$MODE" == runner ]] && OUT="$CACHE/runner"
for path in "$CACHE" "$OUT"; do
  case "$(realpath -m -- "$path")/" in
    "$(realpath -- "$ROOT")/"*) echo 'Cache/runtime paths must be outside the plugin checkout.' >&2; exit 1 ;;
  esac
done
printf 'Build cache: %s\nRuntime: %s\n' "$CACHE" "$OUT"
export DO_NOT_TRACK=1
cargo build --manifest-path "$ROOT/Cargo.toml" --target-dir "$CACHE/target" --release --locked
mkdir -p "$OUT/bin" "$OUT/backend"
# Rename rather than overwrite an executable that may still be running.
install -m 755 "$CACHE/target/release/omarsket" "$OUT/bin/omarsket.new"
mv -f -- "$OUT/bin/omarsket.new" "$OUT/bin/omarsket"
install -m 755 "$CACHE/target/release/omarsket-input" "$OUT/bin/omarsket-input.new"
mv -f -- "$OUT/bin/omarsket-input.new" "$OUT/bin/omarsket-input"
install -m 644 "$ROOT/backend/worker.py" "$OUT/backend/worker.py"
install -m 644 "$ROOT/backend/keymap.py" "$OUT/backend/keymap.py"
uv venv --python "$(command -v python3)" --allow-existing "$OUT/backend/.venv"
WA_PIN=83b7e61ce0eaefae061d9550362c7ae0a71e2318
if [[ -n ${WA_SOURCE:-} ]]; then
  [[ "$(git -C "$WA_SOURCE" rev-parse HEAD)" == "$WA_PIN" ]] || { echo 'WA_SOURCE must match the pinned revision in backend/requirements.txt.' >&2; exit 1; }
  [[ -z "$(git -C "$WA_SOURCE" status --porcelain)" ]] || { echo 'WA_SOURCE has uncommitted changes.' >&2; exit 1; }
  uv pip install --python "$OUT/backend/.venv/bin/python" "$WA_SOURCE"
else
  uv pip install --python "$OUT/backend/.venv/bin/python" -r "$ROOT/backend/requirements.txt"
fi
printf 'Initialized: %s\n' "$OUT"
if [[ "$MODE" == plugin ]]; then
  echo 'Enable with: omarchy plugin enable io.github.commanderturtle.omarsket'
else
  printf 'Run with: %s/bin/omarsket run "Schedule name"\n' "$OUT"
fi
