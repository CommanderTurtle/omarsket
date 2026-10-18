# Omarsket

Macro schedules for Omarchy Quattro / Hyprland. Rust task engine and a private
[Wayland-automation](https://github.com/CommanderTurtle/Wayland-automation) backend.
No network listener, telemetry or separately installed service.

## Install

Requires Omarchy Quattro. Additional build/file-dialog packages:

```sh
sudo pacman -S --needed rust gcc uv zenity
omarchy plugin add https://github.com/CommanderTurtle/omarsket.git
```

Omarchy supplies Python/Git and the Hyprland/input, clipboard, capture, audio and
session tools listed in the manifest. `grim` is used only by the backend's existing
screenshot actions; it is not bundled or installed here. `zenity` is the file
picker. `wayland-utils` is not required on Hyprland.

Choose **No** at the enable prompt, then initialize and enable:

```sh
cd ~/.config/omarchy/plugins/io.github.commanderturtle.omarsket
bash build-plugin.sh
omarchy plugin enable io.github.commanderturtle.omarsket
```

Initialization builds the locked Rust dependencies and installs the pinned WA fork
from `backend/requirements.txt` into a private Python environment. Runtime files
live in `~/.local/share/omarsket/runtime`; build files in `~/.cache/omarsket`
(`XDG_DATA_HOME` / `XDG_CACHE_HOME` are respected). It never installs system packages, copies
plugins or edits shell configuration. Normal startup needs no downloads.
For an unpublished WA checkout, set `WA_SOURCE=/absolute/path/to/Wayland-automation`.

## Use

Click the bar icon, create a schedule, add actions, save and schedule it. Right-click
the icon to stop all runs. Tools checks input capabilities. Imported schedules
require explicit permission for system commands and startup execution.

Schedules are individual `.omscht` files under `~/.local/share/omarsket/schedules`
(or `$XDG_DATA_HOME`). Existing `.scht` files there or in `saved_tasks/` convert on
opening without removing originals. File provides import/export and collection
JSON; Data manages reusable snippets. Optional runner: `bash build-runner.sh`.

Local controllers can share the plugin's queue through the owner-only Unix socket
`$XDG_RUNTIME_DIR/omarsket/control.sock`. Set the plugin setting `controlSocket:false`
to disable it, or a private absolute path to override it. Standalone sessions opt
in with `--control-socket PATH`. This does not enable HTTP or another input engine.

## Update / remove

Disable before updating, run `omarchy plugin update io.github.commanderturtle.omarsket`,
repeat initialization in its checkout, then enable again.

```sh
omarchy plugin remove io.github.commanderturtle.omarsket
```

Removal unloads the plugin and removes its checkout. To also erase the runtime,
saved schedules/snippets and build cache (including the optional runner):

```sh
rm -rf -- "${XDG_DATA_HOME:-$HOME/.local/share}/omarsket" \
  "${XDG_CACHE_HOME:-$HOME/.cache}/omarsket"
```

Remove any custom schedule directory or launchers separately. AGPL-3.0-only.
