# Omarsket

Tasket-style automation for Omarchy/Hyprland. Native Quickshell panel, Rust task
engine, and a Python [Wayland-automation](https://github.com/CommanderTurtle/Wayland-automation)
backend. No network listener, telemetry, root input hooks, or separate service.

## Build

On Linux, alongside the WA fork at commit `83b7e61`:

```sh
bash build-plugin.sh --install
```

Use `--install-deps` to install Arch build/input packages explicitly. Alternatively
set `WA_SOURCE=/path/to/Wayland-automation`. Without a local checkout, the builder
fetches the pinned fork revision; that commit must be published first.

The optional standalone runner uses the same engine and backend, without the GUI:

```sh
bash build-runner.sh --install
omarsket run "Task name"
```

Both builders produce their packages under `dist/`. The installed plugin runs one
shared, owned task process across the bar's monitors. Closing its panel does not
stop a task. Disabling/reloading the plugin stops its process and releases input.
Right-click the bar icon to stop tasks immediately. An optional Hyprland binding:

```ini
bind = SUPER SHIFT, F10, exec, omarchy-shell io.github.commanderturtle.omarsket stop
```

## Tasks

Create actions in the panel, save, and run. Tasks execute serially with delay,
finite/infinite looping, per-action repetition, nested task calls, progress and
stop controls. Edit/reorder/duplicate actions, record keys in the focused recorder,
capture cursor coordinates, and rescale paths for another resolution. Data stores
reusable text, key maps and cursor paths. Startup execution is opt-in.

Tasket `.scht` imports/exports retain its six action types and embedded data.
Omarsket JSON also stores task delay, loops and permissions. Imported tasks never
enable startup or system commands automatically. New actions such as scroll/move
and type are Omarsket extensions, not portable back to Windows Tasket. Qt's binary
`programData.ini` is not imported: use embedded `.scht` data or Omarsket data JSON.

Linux system actions use `xdg-open`, `wpctl`, `grim`, `systemctl`, and `loginctl`;
program arguments use POSIX quoting. System/file actions require the task's explicit
permission. Windows application paths, device GUIDs and shell commands require
Linux replacements. FN hardware keys cannot be synthesized. Hyprland monitor
coordinates are logical desktop coordinates, including negative origins.

Saved tasks/data are private files under `$XDG_DATA_HOME/omarsket` (normally
`~/.local/share/omarsket`). Exports never overwrite an existing file. A task holds
one virtual input session; stop releases held input. No raw keyboard recording
or membership in the Linux `input` group is required.

## Checks

```sh
cargo test
cargo build
python3 -m unittest discover -s tests -p 'test_*.py'
bun test tests/frontend.test.js
```

The backend check in Tools probes the installed library, commands and actual
Wayland protocol support. Device-free tests do not establish live compositor
compatibility; run the plugin and that check in an Omarchy session before relying
on automation. The WA fork carries its own protocol/input regression suite.

## References

[Tasket](https://github.com/AmirHammouteneEI/ScheduledPasteAndKeys) provides the task
format and interaction premise. [Sill](https://github.com/fernandomenolli/omarchy-sill)
and [OmaGravastar](https://github.com/Dankestrick/OmaGravastar) inform the native
plugin integration; Omarsket's QML and Rust code are independently written.
Dependencies: serde, serde_json, libc, Wayland-automation, and Omarchy's shared QML
components. AGPL-3.0-only; dependencies retain their own terms.
