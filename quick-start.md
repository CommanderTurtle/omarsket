# Quick start

Requires Omarchy Quattro and the [prerequisites](README.md#install). No setup script
is required. Run these commands in a terminal; leave the plugin disabled until step 3.

1. Add the plugin, answering **No** when asked to enable it:

   ```sh
   omarchy plugin add https://github.com/CommanderTurtle/omarsket.git
   cd ~/.config/omarchy/plugins/io.github.commanderturtle.omarsket
   ```

2. Build the engine and install the Python backend with `uv`:

   ```sh
   omarsket_cache="${XDG_CACHE_HOME:-$HOME/.cache}/omarsket"
   omarsket_runtime="${XDG_DATA_HOME:-$HOME/.local/share}/omarsket/runtime"
   export DO_NOT_TRACK=1
   cargo build --release --locked --target-dir "$omarsket_cache/target"
   mkdir -p "$omarsket_runtime/bin" "$omarsket_runtime/backend"
   install -m 755 "$omarsket_cache/target/release/omarsket" "$omarsket_runtime/bin/omarsket.new"
   mv -f -- "$omarsket_runtime/bin/omarsket.new" "$omarsket_runtime/bin/omarsket"
   install -m 644 backend/worker.py backend/keymap.py "$omarsket_runtime/backend/"
   uv venv --python "$(command -v python3)" --allow-existing "$omarsket_runtime/backend/.venv"
   uv pip install --python "$omarsket_runtime/backend/.venv/bin/python" -r backend/requirements.txt
   ```

   [The requirement](backend/requirements.txt) pins our WA fork to an exact Git
   commit. `uv` fetches/builds it in its cache and installs it into this venv—not
   the plugin source tree. No manual WA clone or global Python package install is needed.
   Keep the XDG cache/data paths outside the plugin checkout.

3. Enable the plugin:

   ```sh
   omarchy plugin enable io.github.commanderturtle.omarsket
   ```

For updates, disable the plugin, run `omarchy plugin update io.github.commanderturtle.omarsket`,
repeat step 2 from its checkout, then enable.
The optional `bash build-plugin.sh` performs the same initialization.
