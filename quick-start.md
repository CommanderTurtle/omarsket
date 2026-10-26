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

## Keyboard maps and hotkeys

Active hotkey metadata is read on runtime startup, when opening Keyboard, after
Hyprland reports a completed config reload or active-layout change, and with
**Refresh hotkeys** in the keyboard header. Event bursts are debounced and reads
are serialized. Refresh only reads bindings/XKB rules; it does not reload the
compositor, register hooks, start input routing, or execute the displayed keys.
Failed reads retain the last map with an error instead of pretending no bindings
exist. Macrohelp's heatmap connection is independent of this map.

The board is a full-size ANSI reference, not hardware/firmware discovery. Legends
follow the main keyboard's configured XKB layout when reconstructible; physical
evdev identities, heatmap values and remaps do not change when a legend changes.
Custom virtual keymaps without rule names report unavailable rather than assuming
US. In the binding editor, dots/hover text show bindings for the selected modifiers.
Print, Scroll Lock and Pause have their own physical positions.

In **Macros**, select modifier toggles, then right-click a key (or select it and
choose **Assign**). **Choose .omscht…** imports a validated schedule under an
unused name and selects it as the action. Importing does not run it, append a
binding, or save configuration: review **Append binding** / replacement first,
then **Save**. Existing library schedules are selectable without importing.
Conflict confirmation still covers every layer, not just the visible filter.

**Assignments**, **Inactive defaults**, and **Unassigned keys** provide readable
inventories, with descriptions/actions and chord search. A selected key filters
the list; **All keys** clears that filter. Accent-filled keys are assigned in the
current view, pale dots belong to another view, and warning-tinted keys have a
literal packaged default whose global chord is not active. Missing XKB metadata
is reported as unknown, not as a free key. **Show source** retains the native Lua
editor; **Find in source** opens either its personal file or packaged defaults.

Macros identifies Macrohelp/Omarsket commands; Software groups application/text
commands; Omarchy groups native dispatchers and system commands. These are
read-only command-based views of the same native namespace, not separate stores
or a proof of where arbitrary Lua originated. Named submap assignments are listed
with their scope; board colors and free-key checks concern global/universal
bindings. Defaults are extracted from literal `o.bind`/`o.rebind`/`o.bind_hold` and
`hl.bind` calls without executing Lua; generated/dynamic chords are not guessed.
Switching these filters preserves a draft. Leaving the editor for another layer
asks before discarding it. Closing/unloading the entire plugin is not draft storage.
Virtual input highlights the selected device's remaps; Onboard stays explicitly
read-only rather than pretending it can assign generic firmware bindings.

**Refresh hotkeys** preserves an open draft and invalidates any pending replacement
confirmation. **Reload file** (or Cancel) explicitly rereads the native file;
Save retains its existing external-edit check. Changing keyboard sections returns
the view to the top rather than retaining another section's scroll offset.

The event listener uses the shell's existing
[Hyprland IPC events](https://wiki.hypr.land/IPC/) through
[Quickshell](https://quickshell.org/docs/v0.1.0/types/Quickshell.Hyprland/Hyprland/).

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

The Keyboard heatmap first probes `macrohelpctl health` (a one-shot process, not
either HTTP service); only when that succeeds does it open the shell IPC bridge
(`omarchy-shell -q io.github.commanderturtle.macrohelp heatmap <request>`).
If Macrohelp is unreachable, reopening the Heatmap retries the probe.
Snapshots read cached native aggregates only while Heatmap is visible, refreshing
the cache on a one-second cadence while it is shown.

Update the optional helper with the same commands after stopping it. To remove it:
`sudo rm -- /usr/local/libexec/omarsket-input`. Remove saved profiles separately if wanted.

**Keyboard > Onboard** is separately opt-in. Infer reads already-running OpenRazer,
ratbagd (Logitech/SteelSeries), or OmasteelRGB providers without activating services
or adding packages. It shows reported state/capabilities only; no generic firmware
write is assumed. OmasteelRGB's saved map is not a readback of the controller's memory.
