import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("omarsket_worker", ROOT / "backend/worker.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)
keymap_spec = importlib.util.spec_from_file_location("omarsket_keymap", ROOT / "backend/keymap.py")
keymap = importlib.util.module_from_spec(keymap_spec)
keymap_spec.loader.exec_module(keymap)


class KeymapTests(unittest.TestCase):
    def test_missing_virtual_rules_never_guess_a_default_layout(self):
        with patch.object(keymap.C, "CDLL") as load:
            with self.assertRaises(RuntimeError):
                keymap.describe({"layout": "", "active_keymap": "custom"})
            load.assert_not_called()

    def test_active_group_labels_and_all_group_collision_codes(self):
        lib = Mock()
        lib.xkb_context_new.return_value = 1
        lib.xkb_keymap_new_from_names.return_value = 2
        lib.xkb_keymap_min_keycode.return_value = 38
        lib.xkb_keymap_max_keycode.return_value = 38
        lib.xkb_keymap_num_layouts_for_key.return_value = 2
        lib.xkb_keymap_num_levels_for_key.return_value = 1
        values = [(keymap.C.c_uint * 1)(i) for i in (1, 2)]
        def symbols(mapping, code, group, level, out):
            keymap.C.cast(out, keymap.C.POINTER(keymap.C.POINTER(keymap.C.c_uint)))[0] = values[group]
            return 1
        def name(symbol, out, size):
            text = ("a" if symbol == 1 else "Cyrillic_ef").encode()
            keymap.C.memmove(out, text + b"\0", len(text) + 1)
            return len(text)
        lib.xkb_keymap_key_get_syms_by_level.side_effect = symbols
        lib.xkb_keysym_get_name.side_effect = name
        lib.xkb_keysym_to_utf32.side_effect = lambda sym: ord("a" if sym == 1 else "ф")
        with patch.object(keymap.C, "CDLL", return_value=lib):
            result = keymap.describe({"layout": "us,ru", "active_layout_index": 1, "active_keymap": "Russian"})
        self.assertEqual(result["labels"], {"30": "ф"})
        self.assertEqual(result["codes"], {"a": [38], "cyrillic_ef": [38], "ф": [38]})
        self.assertEqual(result["layout"], "Russian")
        lib.xkb_keymap_unref.assert_called_once_with(2)
        lib.xkb_context_unref.assert_called_once_with(1)


class BackendTests(unittest.TestCase):
    def test_overlapping_key_timeline(self):
        events = worker.key_events({"10": [100, ["CONTROL_LEFT", "C"]], "50": [10, ["A"]]})
        self.assertEqual(events[0], (10, "down", "CONTROL_LEFT"))
        self.assertEqual(events[-1], (110, "up", "C"))
        self.assertIn((60, "up", "A"), events)

    def test_aliases_and_hardware_key_rejection(self):
        self.assertEqual(worker.key_name("CONTROL_RIGHT"), "ctrl")
        self.assertEqual(worker.key_name("NUMPAD_2"), "KP_2")
        self.assertEqual(worker.key_name("F12"), "F12")
        with self.assertRaises(ValueError): worker.key_name("FN")

    @patch.object(worker, "checked")
    def test_non_force_power_command(self, checked):
        worker.system_action(dict(sysCommandType="shutdown", sysCommandParam1="Don't force"))
        checked.assert_called_once_with(["systemctl", "poweroff"])
        checked.reset_mock()
        worker.system_action(dict(sysCommandType="restart", sysCommandParam1="Force"))
        checked.assert_called_once_with(["systemctl", "reboot", "--force"])

    @patch.object(worker.subprocess, "Popen")
    def test_program_arguments_are_not_a_shell(self, popen):
        worker.system_action(dict(sysCommandType="executeprogram", sysCommandParam1="/path with spaces/app", sysCommandParam2='--name "two words"'))
        self.assertEqual(popen.call_args.args[0], ["/path with spaces/app", "--name", "two words"])
        self.assertNotIn("shell", popen.call_args.kwargs)

    def test_dangerous_delete_and_bad_values_rejected(self):
        with self.assertRaises(ValueError):
            worker.system_action(dict(sysCommandType="deletefolder", sysCommandParam1=str(Path.home())))
        with self.assertRaises(ValueError):
            worker.system_action(dict(sysCommandType="changeaudiodevice", sysCommandParam1="id;command"))
        with self.assertRaises(ValueError):
            worker.system_action(dict(sysCommandType="openurl", sysCommandParam1="javascript:alert(1)"))

    def test_scroll_and_mouse_counts(self):
        inputs = Mock()
        worker.perform(dict(type="click", x=20, y=30, button="middle", count=2), lambda: inputs)
        self.assertEqual(inputs.mouse.click.call_count, 2)
        worker.perform(dict(type="scroll", vertical=-2, horizontal=1), lambda: inputs)
        inputs.mouse.scroll.assert_called_once_with(-2, 1)

    @patch.object(worker.time, "sleep")
    @patch.object(worker, "_detect_scale")
    @patch.object(worker, "checked")
    def test_pointer_actions_reuse_startup_scale(self, checked, detect_scale, sleep):
        checked.return_value.stdout = '{"x": 10, "y": 20}'
        inputs = worker.Input.__new__(worker.Input)
        inputs.keyboard = Mock()
        inputs._mouse = Mock()
        with patch.object(worker, "SCALE", 2):
            for action in (
                dict(type="move", x=100, y=200),
                dict(type="move", x=200, y=300, duration_ms=32),
                dict(type="cursormovements", cursormovsmap=[[0, 0, 100, 200], [0, 32, 200, 300]]),
                dict(type="click", x=100, y=200, button="left"),
                dict(type="scroll", vertical=1),
                dict(type="cursor"),
            ):
                worker.perform(action, lambda: inputs)
        detect_scale.assert_not_called()
        self.assertTrue(checked.called)
        for invocation in checked.call_args_list:
            self.assertEqual(invocation.args[0], ["hyprctl", "-j", "cursorpos"])
        inputs.mouse.click.assert_any_call(50, 100, "nothing")
        inputs.mouse.click.assert_any_call(100, 150, "nothing")
        inputs.mouse.click.assert_any_call(50, 100, "left")

    @patch.object(worker, "checked")
    def test_explicit_screenshot_actions_keep_tasket_semantics(self, checked):
        checked.return_value.stdout = b"png"
        worker.system_action(dict(sysCommandType="printscreen"))
        self.assertEqual(checked.call_args_list[0].args[0], ["grim", "-"])
        self.assertEqual(checked.call_args_list[1].args[0], ["wl-copy", "--type", "image/png"])
        self.assertEqual(checked.call_args_list[1].kwargs["input"], b"png")

    def test_no_ocr_runtime_or_hyprland_wayland_info_requirement(self):
        source = (ROOT / "backend/worker.py").read_text(encoding="utf-8")
        self.assertNotIn("tesseract", source)
        self.assertNotIn("paddle", source)
        self.assertNotIn('"wayland-info"', source)
        manifest = json.loads((ROOT / "manifest.json").read_text())
        self.assertNotIn("wayland-info", [item["name"] for item in manifest["dependencies"]])

    @patch.object(worker.time, "sleep")
    @patch.object(worker, "checked")
    def test_paste_uses_omarchy_clipboard_insertion_without_an_ocr_engine(self, checked, sleep):
        inputs = Mock()
        worker.perform(dict(type="paste", content="--copy-only\nUnicode: 🐢"), lambda: inputs)
        checked.assert_called_once_with(["wl-copy", "--type", "text/plain;charset=utf-8"],
                                        input="--copy-only\nUnicode: 🐢".encode())
        inputs.keyboard.hotkey.assert_called_once_with("super", "v")


if __name__ == "__main__": unittest.main()
