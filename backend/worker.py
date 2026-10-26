"""Owned JSON-lines input worker. No network, telemetry, or input-device access."""
import json
import logging
import math
import os
from pathlib import Path
import re
import shlex
import shutil
import signal
import subprocess
import sys
import threading
import time

ALIASES = {
    "CONTROL_LEFT": "ctrl", "CONTROL_RIGHT": "Control_R", "SHIFT_LEFT": "shift",
    "SHIFT_RIGHT": "Shift_R", "ALT_LEFT": "alt", "ALT_GR": "ISO_Level3_Shift",
    "WINDOWS": "super", "ENTER": "Return", "ESCAPE": "Escape",
    "BACKSPACE": "BackSpace", "SPACE": "space", "CAPS_LOCK": "Caps_Lock",
    "SCROLL_LOCK": "Scroll_Lock", "UP_ARROW": "Up", "DOWN_ARROW": "Down",
    "LEFT_ARROW": "Left", "RIGHT_ARROW": "Right", "PAGE_UP": "Page_Up",
    "PAGE_DOWN": "Page_Down", "PRINT_SCREEN": "Print", "VOLUME_UP": "XF86AudioRaiseVolume",
    "VOLUME_DOWN": "XF86AudioLowerVolume", "VOLUME_MUTE": "XF86AudioMute",
    "NEXT_TRACK": "XF86AudioNext", "PREVIOUS_TRACK": "XF86AudioPrev",
    "PAUSE": "Pause", "DELETE": "Delete", "INSERT": "Insert", "HOME": "Home",
    "END": "End", "TAB": "Tab", "NUM_LOCK": "Num_Lock",
    "NUMPAD_ENTER": "KP_Enter", "SPACEBAR": "space", "PRINT": "Print",
    "+": "plus", "-": "minus", "*": "asterisk", "/": "slash",
    ".": "period", ",": "comma",
    ";:": "semicolon", "/?": "slash", "`~": "grave", "[{": "bracketleft",
    "\\|": "backslash", "]}": "bracketright", "'\"": "apostrophe",
}
BUTTONS = {"LEFT_MOUSE": 0x110, "RIGHT_MOUSE": 0x111, "MIDDLE_MOUSE": 0x112,
           "XBUTTON_1": 0x113, "XBUTTON_2": 0x114}


def key_name(key):
    if key == "FN":
        raise ValueError("FN is a hardware key and cannot be synthesized")
    if key in ALIASES:
        return ALIASES[key]
    if key.startswith("NUMPAD_") and key[7:].isdigit():
        return "KP_" + key[7:]
    if re.fullmatch(r"F\d{1,2}", key):
        return key
    return key.lower() if len(key) == 1 else key


def key_events(mapping):
    """Translate Tasket pressed-time -> [hold-ms, keys] without losing overlaps."""
    events = []
    for timestamp, pair in mapping.items():
        start, hold, keys = int(timestamp), pair[0], pair[1]
        if start < 0 or hold < 0:
            raise ValueError("Negative key timing")
        for key in keys:
            events.append((start, "down", key))
            events.append((start + hold, "up", key))
    return sorted(events, key=lambda event: (event[0], event[1] == "up"))


def checked(args, **kwargs):
    return subprocess.run(args, check=True, timeout=kwargs.pop("timeout", 30),
                          capture_output=True, **kwargs)


def _detect_scale():
    """Read monitor scale once at startup. Physical = logical * scale."""
    try:
        result = json.loads(checked(["hyprctl", "-j", "monitors"]).stdout)
        if isinstance(result, list) and result:
            return float(result[0].get("scale", 1.0))
    except Exception:
        pass
    return 1.0

SCALE = _detect_scale()

def cursor():
    result = json.loads(checked(["hyprctl", "-j", "cursorpos"]).stdout)
    return int(result["x"] * SCALE), int(result["y"] * SCALE)


class Input:
    def __init__(self):
        from wayland_automation import Keyboard
        logging.getLogger("wayland_automation.mouse_controller").setLevel(logging.WARNING)
        self.keyboard = Keyboard()
        self._mouse = None

    @property
    def mouse(self):
        if self._mouse is None:
            from wayland_automation import Mouse
            self._mouse = Mouse()
        return self._mouse

    def close(self):
        try:
            self.keyboard.close()
        finally:
            if self._mouse is not None:
                self._mouse.close()

    def move(self, x, y, milliseconds=0):
        if milliseconds:
            start_x, start_y = cursor()
            steps = max(1, math.ceil(milliseconds / 16))
            begin = time.monotonic()
            for step in range(1, steps + 1):
                fraction = step / steps
                px = round(start_x + (x - start_x) * fraction)
                py = round(start_y + (y - start_y) * fraction)
                self.mouse.click(int(px / SCALE), int(py / SCALE), "nothing")
                time.sleep(max(0, begin + fraction * milliseconds / 1000 - time.monotonic()))
        else:
            self.mouse.click(int(x / SCALE), int(y / SCALE), "nothing")

    def timeline(self, events):
        """One virtual keyboard alongside one persistent pointer for mixed input."""
        keyboard = [(at, kind, key_name(key)) for at, kind, key in events
                    if key not in BUTTONS and not key.startswith("MOUSE_WHEEL_")]
        pointer = [(at, kind, key) for at, kind, key in events
                   if key in BUTTONS or key.startswith("MOUSE_WHEEL_")]
        if not pointer:
            self.keyboard.sequence(keyboard)
            return
        errors = []
        def type_keys():
            try:
                self.keyboard.sequence(keyboard)
            except Exception as error:
                errors.append(error)
        begin = time.monotonic()
        thread = threading.Thread(target=type_keys)
        thread.start()
        held = set()
        try:
            for at, kind, key in pointer:
                time.sleep(max(0, begin + at / 1000 - time.monotonic()))
                if errors:
                    raise errors[0]
                if key in BUTTONS:
                    self.mouse.button(BUTTONS[key], kind == "down")
                    if kind == "down": held.add(key)
                    else: held.discard(key)
                elif kind == "down":
                    self.mouse.scroll(vertical=-1 if key == "MOUSE_WHEEL_UP" else 1)
        finally:
            for key in held:
                self.mouse.button(BUTTONS[key], False)
            thread.join()
        if errors:
            raise errors[0]

    def path(self, movements, keys):
        if not movements:
            return
        delay, duration, x, y = movements[0]
        time.sleep(delay / 1000)
        self.move(x, y, duration)
        total = sum(delay + duration for delay, duration, _, _ in movements[1:])
        keyboard = [key_name(k) for k in keys if k not in BUTTONS]
        if any(k.startswith("MOUSE_WHEEL_") for k in keyboard):
            raise ValueError("Use a key sequence for wheel events, not a held drag key")
        errors = []
        def hold_keys():
            try:
                self.keyboard.sequence([(0, "down", k) for k in keyboard]
                                       + [(total, "up", k) for k in reversed(keyboard)])
            except Exception as error:
                errors.append(error)
        thread = threading.Thread(target=hold_keys)
        thread.start()
        try:
            for key in keys:
                if key in BUTTONS: self.mouse.button(BUTTONS[key], True)
            for delay, duration, x, y in movements[1:]:
                time.sleep(delay / 1000)
                if errors: raise errors[0]
                self.move(x, y, duration)
        finally:
            for key in keys:
                if key in BUTTONS: self.mouse.button(BUTTONS[key], False)
            thread.join()
        if errors: raise errors[0]


def destination(raw, increment=False):
    path = Path(raw).expanduser().absolute()
    if increment:
        index, original = 1, path
        while path.exists():
            path = original.with_name(f"{original.stem} ({index}){original.suffix}")
            index += 1
    return path


def system_action(action):
    command, p1, p2 = action["sysCommandType"], action.get("sysCommandParam1", ""), action.get("sysCommandParam2", "")
    if command == "executeprogram":
        process = subprocess.Popen([p1, *shlex.split(p2)], stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                   start_new_session=True)
        return {"pid": process.pid}
    if command in ("shutdown", "restart"):
        checked(["systemctl", "poweroff" if command == "shutdown" else "reboot"]
                + (["--force"] if p1 == "Force" else []))
    elif command == "logoff":
        checked(["loginctl", "terminate-user", str(os.getuid())])
    elif command == "quitselfprogram":
        return {"quit": True}
    elif command in ("openfile", "openfolder", "openurl"):
        if command == "openurl" and not re.match(r"^(https?|mailto|file):", p1):
            raise ValueError("Unsupported URL scheme")
        checked(["xdg-open", p1 if command == "openurl" else str(destination(p1))])
    elif command == "changevolume":
        volume = int(p1)
        if not 0 <= volume <= 100: raise ValueError("Volume must be 0–100")
        checked(["wpctl", "set-volume", "@DEFAULT_AUDIO_SINK@", str(volume / 100)])
    elif command == "changeaudiodevice":
        if not p1.isdecimal(): raise ValueError("Use a PipeWire device ID from wpctl status")
        checked(["wpctl", "set-default", p1])
    elif command == "killprocess":
        checked(["pkill", "-u", str(os.getuid()), "-x", "--", p1])
    elif command in ("createfolder", "createfile", "copyfile", "deletefolder", "deletefile"):
        path = destination(p1, p2 == "Auto-rename if exists" and command == "createfile")
        if command in ("deletefolder", "deletefile"):
            resolved = path.resolve()
            if resolved in {Path.home().resolve(), *Path.home().resolve().parents}:
                raise ValueError("Refusing to delete a home/root directory")
            if command == "deletefolder":
                if path.is_symlink(): raise ValueError("Refusing recursive deletion through a symlink")
                shutil.rmtree(path)
            else: path.unlink()
        elif command == "createfolder": path.mkdir(parents=True, exist_ok=True)
        elif command == "createfile":
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"")
        else: shutil.copy2(path, destination(p2))
    elif command in ("screenshot", "printscreen"):
        if command == "printscreen":
            image = checked(["grim", "-"]).stdout
            checked(["wl-copy", "--type", "image/png"], input=image)
        else:
            path = destination(p1, p2 == "Auto-rename if exists")
            path.parent.mkdir(parents=True, exist_ok=True)
            checked(["grim", str(path)])
    else: raise ValueError(f"Unsupported Linux command: {command}")
    return {}


def doctor():
    requirements = ["hyprctl", "wtype", "wl-copy", "grim", "wpctl", "xdg-open"]
    info = {"commands": {name: bool(shutil.which(name)) for name in requirements},
            "wayland": bool(os.environ.get("WAYLAND_DISPLAY")), "library": False}
    try:
        import wayland_automation as wa
        logging.getLogger("wayland_automation.mouse_controller").setLevel(logging.WARNING)
        info["library"] = True
        info["library_version"] = wa.__version__
        with wa.Mouse() as mouse:
            info["protocols"] = mouse.protocols_found
            info["keyboard"] = "zwp_virtual_keyboard_manager_v1" in mouse.protocols_found
        from wayland_automation.utils.screen_resolution import get_geometry
        info["geometry"] = get_geometry()
        info["pointer"] = True
    except Exception as error:
        info["pointer"] = False
        info["error"] = str(error)
    return info


def perform(action, get_input):
    kind = action["type"]
    if kind == "doctor": return doctor()
    if kind == "cursor":
        x, y = cursor()
        return {"x": x, "y": y}
    if kind == "systemcommand": return system_action(action)
    inputs = get_input()
    for _ in range(action.get("loop", 1)):
        if kind == "paste":
            checked(["wl-copy", "--type", "text/plain;charset=utf-8"],
                    input=action.get("content", "").encode())
            # Same insertion path used by Omarchy's clipboard paste helpers.
            time.sleep(0.15)
            inputs.keyboard.hotkey("super", "v")
            time.sleep(0.1)  # Allow the target client to request the clipboard.
        elif kind == "type": inputs.keyboard.typewrite(action["text"], action.get("interval_ms", 0) / 1000)
        elif kind == "hotkey": inputs.keyboard.hotkey(*(key_name(k) for k in action["keys"]))
        elif kind == "keyssequence": inputs.timeline(key_events(action.get("keysmap", {})))
        elif kind == "cursormovements": inputs.path(action.get("cursormovsmap", []), action.get("optionalkeysstroke", []))
        elif kind == "move": inputs.move(action["x"], action["y"], action.get("duration_ms", 0))
        elif kind == "click":
            for _ in range(action.get("count", 1)):
                inputs.mouse.click(int(action["x"] / SCALE), int(action["y"] / SCALE), action["button"])
        elif kind == "scroll": inputs.mouse.scroll(action.get("vertical", 0), action.get("horizontal", 0))
        else: raise ValueError(f"Unknown input action: {kind}")
    return {}


def main():
    if sys.platform != "linux": raise RuntimeError("The live backend requires Linux/Wayland")
    import fcntl
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if not runtime: raise RuntimeError("XDG_RUNTIME_DIR is not set")
    lock_path = Path(runtime) / "omarsket-input.lock"
    lock = os.open(lock_path, os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    inputs = None
    def get_input():
        nonlocal inputs
        if inputs is None:
            try: fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError: raise RuntimeError("Another Omarsket task is using the input devices")
            inputs = Input()
        return inputs
    def stop(signum, frame):
        raise InterruptedError("Stopped")
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    try:
        while True:
            line = sys.stdin.buffer.readline(2 * 1024 * 1024 + 1)
            if not line: break
            try:
                if len(line) > 2 * 1024 * 1024: raise ValueError("Action exceeds 2 MiB")
                result = perform(json.loads(line), get_input)
                print(json.dumps({"ok": True, "result": result}), flush=True)
            except InterruptedError: break
            except Exception as error:
                # Do not echo macro contents or subprocess arguments into logs.
                message = (f"{error.cmd[0]} exited with status {error.returncode}"
                           if isinstance(error, subprocess.CalledProcessError) else str(error))
                print(json.dumps({"ok": False, "error": message}), flush=True)
    finally:
        if inputs is not None: inputs.close()
        os.close(lock)


if __name__ == "__main__":
    main()
