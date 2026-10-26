# Updating (after pull)

One-shot re-initialization after `git pull` in this checkout.
Prerequisite: initial setup done once per `quick-start.md` (plugin added, built, enabled).

Usage, from the checkout root:

    bash updating/after-pull.sh

Sequence (the canonical flow, nothing else):

1. `omarchy plugin disable io.github.commanderturtle.omarsket`
2. `cargo build --release --locked --target-dir "${XDG_CACHE_HOME:-$HOME/.cache}/omarsket/target"`
3. Install `omarsket` and `omarsket-input` into `"${XDG_DATA_HOME:-$HOME/.local/share}/omarsket/runtime/bin/"`
   (via `.new` + `mv -f`, never overwriting a running binary in place)
4. Install `backend/worker.py` and `backend/keymap.py` into `"$omarsket_runtime/backend/"`
5. `uv venv --python "$(command -v python3)" --allow-existing "$omarsket_runtime/backend/.venv"`
6. `uv pip install --python "$omarsket_runtime/backend/.venv/bin/python" -r backend/requirements.txt`
   (re-pins the Wayland-automation fork to the exact commit in `requirements.txt`)
7. `omarchy plugin enable io.github.commanderturtle.omarsket`

No tests. If the shell still loads a stale client afterwards,
`omarchy restart shell` picks the new runtime up (macrohelp's
`updating/after-pull.sh` does that as its final step).

Run this before macrohelp's `updating/after-pull.sh` after a pull.
