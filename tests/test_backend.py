import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("omarsket_worker", ROOT / "backend/worker.py")
worker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(worker)


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
