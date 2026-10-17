#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:?plugin or runner required}"
shift
[[ "$MODE" == plugin || "$MODE" == runner ]] || { echo 'Expected plugin or runner' >&2; exit 1; }
[[ "$(uname -s)" == Linux ]] || { echo 'Build on Linux (Arch/Omarchy).' >&2; exit 1; }
INSTALL=0
for arg in "$@"; do
  case "$arg" in
    --install) INSTALL=1 ;;
    --install-deps)
      command -v pacman >/dev/null || { echo '--install-deps requires Arch pacman' >&2; exit 1; }
      sudo pacman -S --needed rust uv python wtype wl-clipboard wayland-utils grim zenity
      ;;
    *) printf 'Unknown option: %s\n' "$arg" >&2; exit 1 ;;
  esac
done
for tool in cargo uv python3; do
  command -v "$tool" >/dev/null || { printf 'Missing %s. On Arch: sudo pacman -S rust uv python\n' "$tool" >&2; exit 1; }
done
export DO_NOT_TRACK=1
cargo build --manifest-path "$ROOT/Cargo.toml" --release --locked
NAME=omarsket
[[ "$MODE" == runner ]] && NAME=omarsket-runner
OUT="$ROOT/dist/$NAME"
mkdir -p "$OUT/bin" "$OUT/backend"
install -m 755 "$ROOT/target/release/omarsket" "$OUT/bin/omarsket"
install -m 644 "$ROOT/backend/worker.py" "$OUT/backend/worker.py"
uv venv --python "$(command -v python3)" --allow-existing "$OUT/backend/.venv"
WA_PIN=83b7e61ce0eaefae061d9550362c7ae0a71e2318
DEFAULT_WA="$ROOT/../Wayland-automation"
[[ -f "$ROOT/../wayland-automation/setup.py" ]] && DEFAULT_WA="$ROOT/../wayland-automation"
WA_SOURCE="${WA_SOURCE:-$DEFAULT_WA}"
if [[ -f "$WA_SOURCE/setup.py" ]]; then
  [[ "$(git -C "$WA_SOURCE" rev-parse HEAD)" == "$WA_PIN" ]] || { echo 'Local WA_SOURCE does not match the reviewed commit.' >&2; exit 1; }
  [[ -z "$(git -C "$WA_SOURCE" status --porcelain)" ]] || { echo 'WA_SOURCE contains uncommitted changes.' >&2; exit 1; }
  uv pip install --python "$OUT/backend/.venv/bin/python" "$WA_SOURCE"
else
  uv pip install --python "$OUT/backend/.venv/bin/python" -r "$ROOT/backend/requirements.txt"
fi
if [[ "$MODE" == plugin ]]; then
  cp "$ROOT/manifest.json" "$OUT/manifest.json"
  cp -R "$ROOT/qml" "$OUT/"
  if (( INSTALL )); then
    command -v omarchy >/dev/null || { echo 'Omarchy is required for plugin installation.' >&2; exit 1; }
    DEST="${XDG_CONFIG_HOME:-$HOME/.config}/omarchy/plugins/io.github.commanderturtle.omarsket"
    mkdir -p "$DEST"
    cp -R "$OUT/." "$DEST/"
    omarchy plugin validate "$DEST"
    QMLLINT="$(command -v qmllint || true)"
    [[ -n "$QMLLINT" ]] || QMLLINT=/usr/lib/qt6/bin/qmllint
    if [[ -x "$QMLLINT" ]]; then
      SHELL_DIR="${OMARCHY_PATH:-/usr/share/omarchy}/shell"
      [[ -d "$SHELL_DIR" ]] || SHELL_DIR="$HOME/.local/share/omarchy/shell"
      [[ -d "$SHELL_DIR/Ui" && -d "$SHELL_DIR/Commons" ]] || { echo 'Set OMARCHY_PATH to the installed shell sources for QML validation.' >&2; exit 1; }
      # Quickshell's qs alias needs its directory layout when linting offline.
      QML_IMPORTS="$(mktemp -d /tmp/omarsket-qml.XXXXXXXX)"
      trap '[[ "$QML_IMPORTS" == /tmp/omarsket-qml.* ]] && rm -rf -- "$QML_IMPORTS"' EXIT
      mkdir "$QML_IMPORTS/qs"
      ln -s "$SHELL_DIR/Ui" "$QML_IMPORTS/qs/Ui"
      ln -s "$SHELL_DIR/Commons" "$QML_IMPORTS/qs/Commons"
      "$QMLLINT" -I "$QML_IMPORTS" "$DEST"/qml/*.qml
      [[ "$QML_IMPORTS" == /tmp/omarsket-qml.* ]] && rm -rf -- "$QML_IMPORTS"
      trap - EXIT
    fi
    omarchy plugin enable io.github.commanderturtle.omarsket
  fi
elif (( INSTALL )); then
  DEST="${XDG_DATA_HOME:-$HOME/.local/share}/omarsket/runner"
  mkdir -p "$DEST" "$HOME/.local/bin"
  cp -R "$OUT/." "$DEST/"
  LINK="$HOME/.local/bin/omarsket"
  if [[ -e "$LINK" || -L "$LINK" ]]; then
    [[ -L "$LINK" && "$(readlink "$LINK")" == "$DEST/bin/omarsket" ]] || { echo "$LINK already exists; leaving it unchanged" >&2; exit 1; }
  else
    ln -s "$DEST/bin/omarsket" "$LINK"
  fi
fi
printf 'Built %s: %s\n' "$MODE" "$OUT"
