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
   install -m 755 "$omarsket_cache/target/release/omarsket-input" "$omarsket_runtime/bin/omarsket-input"
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

## Optional physical input routing

Off by default. Uses the system `libevdev` and kernel uinput, not another remapping
package. To make **Keyboard > Virtual input** available after building:

```sh
omarsket_runtime="${XDG_DATA_HOME:-$HOME/.local/share}/omarsket/runtime"
sudo install -d -o root -g root -m 755 /usr/local/libexec
sudo install -o root -g root -m 755 "$omarsket_runtime/bin/omarsket-input" /usr/local/libexec/omarsket-input.new
sudo mv -- /usr/local/libexec/omarsket-input.new /usr/local/libexec/omarsket-input
```

Enable the layer, save a named profile for selected physical keyboards/mice, then
choose **Start · authorize**. Profiles are private files under
`~/.local/share/omarsket/input-profiles` (`XDG_DATA_HOME` respected). The worker drops
root after opening the selected devices. No group, udev rule, login service or
native Omarchy binding is changed. Stop, closing its controller, unplugging a device,
or **Escape+Backspace+Enter** releases the grab. The authorization terminal also
accepts Ctrl+C. Touch/absolute devices stay on their existing native path;
`ts_uinput` is an immutable default, not a keyboard router. Firmware-only Fn keys
need an actual emitted event. Virtual devices are named `Omarsket …`; device-specific
compositor settings may need a matching native rule.
Macrohelp's compositor-level key display and heatmap observe the remapped output.

The Keyboard heatmap connects to Macrohelp independently of task execution. If
Macrohelp starts later or its bridge disconnects, reopen the Keyboard panel or
choose **Retry**. A failed connection is not cached for the rest of the shell
session; a successful connection still uses native aggregate IPC, not repeated
HTTP health checks. Pending heatmap commands are discarded on a transport failure
so reconnecting does not replay a stale Start/End/Delete request.

Update the optional helper with the same commands after stopping it. To remove it:
`sudo rm -- /usr/local/libexec/omarsket-input`. Remove saved profiles separately if wanted.

**Keyboard > Onboard** is separately opt-in. Infer reads already-running OpenRazer,
ratbagd (Logitech/SteelSeries), or OmasteelRGB providers without activating services
or adding packages. It shows reported state/capabilities only; no generic firmware
write is assumed. OmasteelRGB's saved map is not a readback of the controller's memory.
