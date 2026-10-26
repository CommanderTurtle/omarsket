#!/bin/sh
# Re-initialize omarsket after pulling changes into this checkout.
# Run from the checkout root: bash updating/after-pull.sh
# Zero frills: this is the canonical update sequence, no tests.

set -eu

cd "$(dirname "$0")/.."

omarchy plugin disable io.github.commanderturtle.omarsket

omarsket_cache="${XDG_CACHE_HOME:-$HOME/.cache}/omarsket"
omarsket_runtime="${XDG_DATA_HOME:-$HOME/.local/share}/omarsket/runtime"
export DO_NOT_TRACK=1
cargo build --release --locked --target-dir "$omarsket_cache/target"
mkdir -p "$omarsket_runtime/bin" "$omarsket_runtime/backend"
install -m 755 "$omarsket_cache/target/release/omarsket" "$omarsket_runtime/bin/omarsket.new"
mv -f -- "$omarsket_runtime/bin/omarsket.new" "$omarsket_runtime/bin/omarsket"
install -m 755 "$omarsket_cache/target/release/omarsket-input" "$omarsket_runtime/bin/omarsket-input"
install -m 644 backend/worker.py backend/keymap.py "$omarsket_runtime/backend/"
uv venv --python "$(command -v python3)" --allow-existing "$omarsket_runtime/backend/.venv"
uv pip install --python "$omarsket_runtime/backend/.venv/bin/python" -r backend/requirements.txt

omarchy plugin enable io.github.commanderturtle.omarsket
